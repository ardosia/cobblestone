use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use cobblestone_core::RuntimeId;
use cobblestone_runtime_probe::{
    RoutedMessage, RuntimeCommand, RuntimeCompletion, RuntimeProcess,
};

const TOTAL_MESSAGES: u64 = 1_000_000;
const MAILBOX_CAPACITY: usize = 64;
const DEADLOCK_WATCHDOG: Duration = Duration::from_secs(600);

fn main() -> Result<(), Box<dyn Error>> {
    let php = std::env::var_os("PHP_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("php"));
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tools/php-runtime-worker.php");

    let runtime_one = RuntimeId::new(1).ok_or_else(|| io::Error::other("runtime id 1 invalid"))?;
    let runtime_two = RuntimeId::new(2).ok_or_else(|| io::Error::other("runtime id 2 invalid"))?;
    let first = RuntimeProcess::spawn(
        runtime_one,
        &php,
        &worker,
        MAILBOX_CAPACITY,
        MAILBOX_CAPACITY,
    )?;
    let second = RuntimeProcess::spawn(
        runtime_two,
        &php,
        &worker,
        MAILBOX_CAPACITY,
        MAILBOX_CAPACITY,
    )?;

    let mut seen = vec![false; usize::try_from(TOTAL_MESSAGES)?];
    let mut next_sequence = 1_u64;
    let mut received = 0_u64;
    let mut backpressure_events = 0_u64;
    let started = Instant::now();

    while received < TOTAL_MESSAGES {
        if started.elapsed() > DEADLOCK_WATCHDOG {
            return Err(io::Error::other(format!(
                "message torture exceeded deadlock watchdog with sent={} received={received}",
                next_sequence - 1
            ))
            .into());
        }

        let mut submitted_any = false;
        while next_sequence <= TOTAL_MESSAGES {
            let target = target_for(next_sequence, runtime_one, runtime_two);
            let message = RoutedMessage {
                sequence: next_sequence,
                producer: runtime_one,
                target,
                checksum: checksum(next_sequence, runtime_one, target),
            };
            let runtime = if target == runtime_one {
                &first
            } else {
                &second
            };

            match runtime.try_submit(RuntimeCommand::Message(message)) {
                Ok(()) => {
                    next_sequence += 1;
                    submitted_any = true;
                }
                Err(TrySendError::Full(RuntimeCommand::Message(returned))) => {
                    if returned != message {
                        return Err(io::Error::other(
                            "full mailbox returned a different message",
                        )
                        .into());
                    }
                    backpressure_events += 1;
                    break;
                }
                Err(TrySendError::Full(_)) => {
                    return Err(io::Error::other(
                        "full mailbox returned an unexpected command kind",
                    )
                    .into());
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(io::Error::other("runtime command mailbox disconnected").into());
                }
            }
        }

        let first_drained = drain_runtime(
            &first,
            runtime_one,
            runtime_one,
            runtime_two,
            &mut seen,
            &mut received,
        )?;
        let second_drained = drain_runtime(
            &second,
            runtime_two,
            runtime_one,
            runtime_two,
            &mut seen,
            &mut received,
        )?;

        if !submitted_any && first_drained + second_drained == 0 {
            thread::yield_now();
        }
    }

    if next_sequence != TOTAL_MESSAGES + 1 {
        return Err(io::Error::other(format!(
            "received all messages before submitting all messages: next={next_sequence}"
        ))
        .into());
    }
    if seen.iter().any(|seen| !seen) {
        return Err(io::Error::other("message torture detected at least one lost sequence").into());
    }
    if backpressure_events == 0 {
        return Err(io::Error::other(
            "bounded runtime mailboxes never surfaced backpressure under forced saturation",
        )
        .into());
    }

    let first_status = first.shutdown()?;
    let second_status = second.shutdown()?;
    if !first_status.success() || !second_status.success() {
        return Err(io::Error::other(format!(
            "unclean runtime shutdown: first={first_status:?} second={second_status:?}"
        ))
        .into());
    }

    let elapsed = started.elapsed();
    let rate = TOTAL_MESSAGES as f64 / elapsed.as_secs_f64();
    println!(
        "message-torture: messages={TOTAL_MESSAGES} received={received} backpressure_events={backpressure_events} elapsed_ms={} messages_per_second={rate:.2}",
        elapsed.as_millis()
    );
    Ok(())
}

fn drain_runtime(
    runtime: &RuntimeProcess,
    mailbox_owner: RuntimeId,
    producer: RuntimeId,
    alternate: RuntimeId,
    seen: &mut [bool],
    received: &mut u64,
) -> Result<usize, Box<dyn Error>> {
    let mut drained = 0_usize;
    loop {
        match runtime.try_recv_completion() {
            Ok(RuntimeCompletion::Message(message)) => {
                validate_message(
                    message,
                    mailbox_owner,
                    producer,
                    alternate,
                    seen,
                    received,
                )?;
                drained += 1;
            }
            Ok(other) => {
                return Err(io::Error::other(format!(
                    "unexpected non-message completion during torture: {other:?}"
                ))
                .into());
            }
            Err(TryRecvError::Empty) => return Ok(drained),
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("runtime completion mailbox disconnected").into());
            }
        }
    }
}

fn validate_message(
    message: RoutedMessage,
    mailbox_owner: RuntimeId,
    producer: RuntimeId,
    alternate: RuntimeId,
    seen: &mut [bool],
    received: &mut u64,
) -> Result<(), Box<dyn Error>> {
    if message.sequence == 0 || message.sequence > TOTAL_MESSAGES {
        return Err(io::Error::other(format!(
            "out-of-range message sequence {}",
            message.sequence
        ))
        .into());
    }

    let expected_target = target_for(message.sequence, producer, alternate);
    if message.producer != producer
        || message.target != expected_target
        || message.target != mailbox_owner
    {
        return Err(io::Error::other(format!(
            "message misrouted or corrupted: {message:?} mailbox_owner={}",
            mailbox_owner.get()
        ))
        .into());
    }

    let expected_checksum = checksum(message.sequence, message.producer, message.target);
    if message.checksum != expected_checksum {
        return Err(io::Error::other(format!(
            "message checksum mismatch at sequence {}",
            message.sequence
        ))
        .into());
    }

    let index = usize::try_from(message.sequence - 1)?;
    if seen[index] {
        return Err(io::Error::other(format!(
            "duplicate completion for sequence {}",
            message.sequence
        ))
        .into());
    }
    seen[index] = true;
    *received += 1;
    Ok(())
}

const fn target_for(sequence: u64, first: RuntimeId, second: RuntimeId) -> RuntimeId {
    if sequence & 1 == 0 { second } else { first }
}

const fn checksum(sequence: u64, producer: RuntimeId, target: RuntimeId) -> u64 {
    sequence.rotate_left(17)
        ^ ((producer.get() as u64) << 32)
        ^ (target.get() as u64)
        ^ 0xd6e8_feb8_6659_fd93
}
