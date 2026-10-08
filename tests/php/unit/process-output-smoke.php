<?php

declare(strict_types=1);

require __DIR__ . '/../integration/ClientOutput.php';

$output = new ClientOutput();
$process = proc_open(
    [PHP_BINARY, '-n', '-r', 'fwrite(STDOUT, "ready\n"); fflush(STDOUT); usleep(1200000); fwrite(STDERR, "done\n");'],
    $output->descriptors(),
    $pipes,
);
if (!is_resource($process)) {
    throw new RuntimeException('failed to launch child process output probe');
}
fclose($pipes[0]);

$stdout = '';
$stderr = '';
$readyWhileRunning = false;
$exitCode = null;
$deadline = hrtime(true) + 5_000_000_000;
try {
    while (hrtime(true) < $deadline) {
        $stdout .= $output->readStdout();
        $stderr .= $output->readStderr();
        $status = proc_get_status($process);
        $readyWhileRunning = $readyWhileRunning || $status['running'] && str_contains($stdout, "ready\n");
        if (!$status['running']) {
            $exitCode = $status['exitcode'];
            break;
        }
        usleep(10_000);
    }
    if ($exitCode === null) {
        proc_terminate($process);
        throw new RuntimeException('client output probe timed out');
    }
    $stdout .= $output->readStdout();
    $stderr .= $output->readStderr();
    if (!$readyWhileRunning || $stdout !== "ready\n" || $stderr !== "done\n" || $exitCode !== 0) {
        throw new RuntimeException("client output probe failed: live={$readyWhileRunning}, exit={$exitCode}, stdout={$stdout}, stderr={$stderr}");
    }
} finally {
    proc_close($process);
    $output->close();
}

fwrite(STDOUT, "process-output-smoke: passed\n");
