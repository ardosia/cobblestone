use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use cobblestone_core::RuntimeId;
use cobblestone_runtime_probe::{
    RoutedMessage, RuntimeCommand, RuntimeCompletion, RuntimeProcess,
};

fn main() -> Result<(), Box<dyn Error>> {
    let php = std::env::var_os("PHP_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("php"));
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tools/php-runtime-worker.php");

    let runtime_one = RuntimeId::new(1).ok_or_else(|| io::Error::other("runtime id 1 invalid"))?;
    let runtime_two = RuntimeId::new(2).ok_or_else(|| io::Error::other("runtime id 2 invalid"))?;

    let first = RuntimeProcess::spawn(runtime_one, &php, &worker, 8, 8)?;
    let second = RuntimeProcess::spawn(runtime_two, &php, &worker, 8, 8)?;

    if first.boot().pid() == second.boot().pid() {
        return Err(io::Error::other("persistent runtimes unexpectedly share one process").into());
    }

    let first_message = RoutedMessage {
        sequence: 1,
        producer: runtime_one,
        target: runtime_one,
        checksum: 0xA11CE,
    };
    let second_message = RoutedMessage {
        sequence: 2,
        producer: runtime_one,
        target: runtime_two,
        checksum: 0xB0B,
    };

    first.try_submit(RuntimeCommand::Message(first_message))?;
    second.try_submit(RuntimeCommand::Message(second_message))?;
    expect_message(&first, first_message)?;
    expect_message(&second, second_message)?;

    first.try_submit(RuntimeCommand::CollectGc { sequence: 3 })?;
    second.try_submit(RuntimeCommand::CollectGc { sequence: 4 })?;
    expect_gc(&first, 3)?;
    expect_gc(&second, 4)?;

    let first_pid = first.boot().pid();
    let second_pid = second.boot().pid();
    let first_shutdown = first.shutdown()?;
    let second_shutdown = second.shutdown()?;
    if !first_shutdown.success() || !second_shutdown.success() {
        return Err(io::Error::other(format!(
            "unclean runtime shutdown: first={first_shutdown:?} second={second_shutdown:?}"
        ))
        .into());
    }

    println!(
        "multiruntime-smoke: runtimes=2 pids={first_pid},{second_pid} bounded_mailboxes=yes gc=enabled shutdown=clean"
    );
    Ok(())
}

fn expect_message(runtime: &RuntimeProcess, expected: RoutedMessage) -> Result<(), Box<dyn Error>> {
    match runtime.recv_completion_timeout(Duration::from_secs(10))? {
        RuntimeCompletion::Message(actual) if actual == expected => Ok(()),
        other => Err(io::Error::other(format!(
            "unexpected message completion: expected={expected:?} actual={other:?}"
        ))
        .into()),
    }
}

fn expect_gc(runtime: &RuntimeProcess, sequence: u64) -> Result<(), Box<dyn Error>> {
    match runtime.recv_completion_timeout(Duration::from_secs(10))? {
        RuntimeCompletion::Gc {
            sequence: actual, ..
        } if actual == sequence => Ok(()),
        other => Err(io::Error::other(format!(
            "unexpected GC completion for sequence {sequence}: {other:?}"
        ))
        .into()),
    }
}
