#![forbid(unsafe_code)]

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cobblestone_core::RuntimeId;

/// Handshake reported by one persistent PHP runtime process.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RuntimeBoot {
    runtime: RuntimeId,
    pid: u32,
}

impl RuntimeBoot {
    pub const fn runtime(&self) -> RuntimeId {
        self.runtime
    }

    pub const fn pid(&self) -> u32 {
        self.pid
    }
}

/// Integrity-carrying diagnostic message routed through one runtime mailbox.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RoutedMessage {
    pub sequence: u64,
    pub producer: RuntimeId,
    pub target: RuntimeId,
    pub checksum: u64,
}

/// Commands accepted by a persistent diagnostic runtime.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RuntimeCommand {
    Message(RoutedMessage),
    CollectGc { sequence: u64 },
}

/// Completions returned by a persistent diagnostic runtime.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RuntimeCompletion {
    Message(RoutedMessage),
    Gc {
        sequence: u64,
        cycles: u32,
        memory_bytes: u64,
    },
}

struct RuntimeIoState {
    child: Option<Child>,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl RuntimeIoState {
    fn wait(mut self) -> io::Result<ExitStatus> {
        let mut child = self
            .child
            .take()
            .ok_or_else(|| io::Error::other("runtime process already consumed"))?;
        child.wait()
    }
}

impl Drop for RuntimeIoState {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Private C003 process-isolated runtime substrate.
///
/// Each instance owns one persistent PHP process plus bounded command and completion mailboxes.
/// Arbitrary PHP/Zend execution stays inside the child process. This is an internal experiment
/// surface, not a plugin API.
pub struct RuntimeProcess {
    boot: RuntimeBoot,
    commands: Option<SyncSender<RuntimeCommand>>,
    completions: Option<Receiver<RuntimeCompletion>>,
    io_thread: Option<JoinHandle<io::Result<ExitStatus>>>,
}

impl RuntimeProcess {
    /// Starts one persistent PHP 8.5 ZTS runtime with bounded mailboxes.
    pub fn spawn(
        runtime: RuntimeId,
        php_binary: impl AsRef<Path>,
        worker_script: impl AsRef<Path>,
        command_capacity: usize,
        completion_capacity: usize,
    ) -> io::Result<Self> {
        if command_capacity == 0 || completion_capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime mailbox capacities must be nonzero",
            ));
        }

        let mut child = Command::new(php_binary.as_ref())
            .arg("-n")
            .arg(worker_script.as_ref())
            .env("COBBLESTONE_RUNTIME_ID", runtime.get().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other("PHP runtime did not expose piped stdin"));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other("PHP runtime did not expose piped stdout"));
            }
        };
        let mut state = RuntimeIoState {
            child: Some(child),
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
        };

        let mut line = String::new();
        if state.stdout.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "PHP runtime closed stdout before its handshake",
            ));
        }
        let boot = parse_boot(&line)?;
        if boot.runtime != runtime {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PHP runtime identity mismatch: expected {}, got {}",
                    runtime.get(),
                    boot.runtime.get()
                ),
            ));
        }

        let (command_tx, command_rx) = sync_channel(command_capacity);
        let (completion_tx, completion_rx) = sync_channel(completion_capacity);
        let io_thread = thread::Builder::new()
            .name(format!("cobblestone-php-runtime-{}", runtime.get()))
            .spawn(move || runtime_loop(state, command_rx, completion_tx))?;

        Ok(Self {
            boot,
            commands: Some(command_tx),
            completions: Some(completion_rx),
            io_thread: Some(io_thread),
        })
    }

    pub const fn boot(&self) -> RuntimeBoot {
        self.boot
    }

    /// Attempts to enqueue a command without blocking.
    pub fn try_submit(&self, command: RuntimeCommand) -> Result<(), TrySendError<RuntimeCommand>> {
        match self.commands.as_ref() {
            Some(sender) => sender.try_send(command),
            None => Err(TrySendError::Disconnected(command)),
        }
    }

    pub fn try_recv_completion(&self) -> Result<RuntimeCompletion, TryRecvError> {
        match self.completions.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => Err(TryRecvError::Disconnected),
        }
    }

    pub fn recv_completion_timeout(
        &self,
        timeout: Duration,
    ) -> Result<RuntimeCompletion, RecvTimeoutError> {
        match self.completions.as_ref() {
            Some(receiver) => receiver.recv_timeout(timeout),
            None => Err(RecvTimeoutError::Disconnected),
        }
    }

    /// Drains accepted commands, sends an explicit stop command, and joins the transport thread.
    pub fn shutdown(mut self) -> io::Result<ExitStatus> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> io::Result<ExitStatus> {
        // Releasing the completion receiver first unblocks a transport thread stalled on the
        // bounded completion queue. Releasing the command sender then lets it drain commands that
        // were already accepted before issuing the protocol stop.
        self.completions.take();
        self.commands.take();

        let Some(thread) = self.io_thread.take() else {
            return Err(io::Error::other("runtime transport thread already joined"));
        };
        thread
            .join()
            .map_err(|_| io::Error::other("runtime transport thread panicked"))?
    }
}

impl Drop for RuntimeProcess {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

const RUNTIME_BATCH_LIMIT: usize = 64;

fn runtime_loop(
    mut state: RuntimeIoState,
    commands: Receiver<RuntimeCommand>,
    completions: SyncSender<RuntimeCompletion>,
) -> io::Result<ExitStatus> {
    while let Ok(first) = commands.recv() {
        let mut batch = Vec::with_capacity(RUNTIME_BATCH_LIMIT);
        batch.push(first);

        while batch.len() < RUNTIME_BATCH_LIMIT {
            match commands.try_recv() {
                Ok(command) => batch.push(command),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }

        for command in &batch {
            write_command(&mut state.stdin, *command)?;
        }
        writeln!(state.stdin, "FLUSH\t{}", batch.len())?;
        state.stdin.flush()?;

        for command in batch.iter().copied() {
            let completion = read_completion(&mut state.stdout)?;
            if !completion_matches(command, completion) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("runtime completion did not match command: {completion:?}"),
                ));
            }
            let _ = completions.send(completion);
        }
        read_flush_ack(&mut state.stdout, batch.len())?;
    }

    writeln!(state.stdin, "STOP\t0")?;
    state.stdin.flush()?;
    let mut line = String::new();
    if state.stdout.read_line(&mut line)? == 0 || line.trim_end() != "STOPPED\t0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid runtime shutdown reply: {line:?}"),
        ));
    }

    state.wait()
}

fn write_command(stdin: &mut BufWriter<ChildStdin>, command: RuntimeCommand) -> io::Result<()> {
    match command {
        RuntimeCommand::Message(message) => writeln!(
            stdin,
            "MSG\t{}\t{}\t{}\t{}",
            message.sequence,
            message.producer.get(),
            message.target.get(),
            message.checksum
        )?,
        RuntimeCommand::CollectGc { sequence } => writeln!(stdin, "GC\t{sequence}")?,
    }
    Ok(())
}

fn read_flush_ack(stdout: &mut BufReader<ChildStdout>, expected: usize) -> io::Result<()> {
    let mut line = String::new();
    if stdout.read_line(&mut line)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "runtime closed stdout before batch acknowledgement",
        ));
    }
    let fields: Vec<&str> = line.trim_end().split('\t').collect();
    if fields.len() != 2 || fields[0] != "FLUSHED" {
        return Err(invalid_data(&format!(
            "invalid runtime batch acknowledgement: {line:?}"
        )));
    }
    let actual = fields[1]
        .parse::<usize>()
        .map_err(|_| invalid_data("runtime batch acknowledgement was not a usize"))?;
    if actual != expected {
        return Err(invalid_data(&format!(
            "runtime batch acknowledgement mismatch: expected={expected} actual={actual}"
        )));
    }
    Ok(())
}

fn read_completion(stdout: &mut BufReader<ChildStdout>) -> io::Result<RuntimeCompletion> {
    let mut line = String::new();
    if stdout.read_line(&mut line)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "runtime closed stdout before replying",
        ));
    }
    parse_completion(&line)
}

fn parse_boot(line: &str) -> io::Result<RuntimeBoot> {
    let fields: Vec<&str> = line.trim_end().split('\t').collect();
    if fields.len() != 4 || fields[0] != "BOOT" || fields[3] != "1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid PHP 8.5 ZTS boot line: {line:?}"),
        ));
    }
    let runtime = parse_runtime(fields[1], "runtime")?;
    let pid = fields[2]
        .parse::<u32>()
        .map_err(|_| invalid_data("boot pid was not a u32"))?;
    if pid == 0 {
        return Err(invalid_data("boot pid was zero"));
    }
    Ok(RuntimeBoot { runtime, pid })
}

fn parse_completion(line: &str) -> io::Result<RuntimeCompletion> {
    let fields: Vec<&str> = line.trim_end().split('\t').collect();
    match fields.as_slice() {
        ["MSG", sequence, producer, target, checksum] => {
            Ok(RuntimeCompletion::Message(RoutedMessage {
                sequence: parse_u64(sequence, "message sequence")?,
                producer: parse_runtime(producer, "message producer")?,
                target: parse_runtime(target, "message target")?,
                checksum: parse_u64(checksum, "message checksum")?,
            }))
        }
        ["GC", sequence, cycles, memory_bytes] => Ok(RuntimeCompletion::Gc {
            sequence: parse_u64(sequence, "GC sequence")?,
            cycles: cycles
                .parse::<u32>()
                .map_err(|_| invalid_data("GC cycle count was not a u32"))?,
            memory_bytes: parse_u64(memory_bytes, "GC memory")?,
        }),
        _ => Err(invalid_data(&format!(
            "unexpected runtime completion line: {line:?}"
        ))),
    }
}

fn parse_runtime(value: &str, name: &str) -> io::Result<RuntimeId> {
    let raw = value
        .parse::<u32>()
        .map_err(|_| invalid_data(&format!("{name} was not a u32")))?;
    RuntimeId::new(raw).ok_or_else(|| invalid_data(&format!("{name} was zero")))
}

fn parse_u64(value: &str, name: &str) -> io::Result<u64> {
    value
        .parse::<u64>()
        .map_err(|_| invalid_data(&format!("{name} was not a u64")))
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

fn completion_matches(command: RuntimeCommand, completion: RuntimeCompletion) -> bool {
    match (command, completion) {
        (RuntimeCommand::Message(expected), RuntimeCompletion::Message(actual)) => {
            expected == actual
        }
        (
            RuntimeCommand::CollectGc { sequence: expected },
            RuntimeCompletion::Gc {
                sequence: actual, ..
            },
        ) => expected == actual,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use cobblestone_core::RuntimeId;

    use super::{
        RoutedMessage, RuntimeCommand, RuntimeCompletion, completion_matches, parse_boot,
        parse_completion,
    };

    #[test]
    fn parses_zts_boot_handshake() {
        let boot = parse_boot("BOOT\t2\t1234\t1\n").expect("valid boot line");
        assert_eq!(boot.runtime().get(), 2);
        assert_eq!(boot.pid(), 1234);
    }

    #[test]
    fn parses_integrity_message_completion() {
        let completion =
            parse_completion("MSG\t99\t1\t2\t123456\n").expect("valid message completion");
        let expected = RoutedMessage {
            sequence: 99,
            producer: RuntimeId::new(1).expect("nonzero"),
            target: RuntimeId::new(2).expect("nonzero"),
            checksum: 123456,
        };
        assert_eq!(completion, RuntimeCompletion::Message(expected));
        assert!(completion_matches(
            RuntimeCommand::Message(expected),
            completion
        ));
    }

    #[test]
    fn rejects_zero_runtime_in_message() {
        assert!(parse_completion("MSG\t1\t0\t2\t3\n").is_err());
    }
}
