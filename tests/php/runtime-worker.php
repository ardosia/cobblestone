<?php

declare(strict_types=1);

if (PHP_MAJOR_VERSION !== 8 || PHP_MINOR_VERSION !== 5) {
    fwrite(STDERR, sprintf("expected PHP 8.5.x, got %s\n", PHP_VERSION));
    exit(2);
}

if (PHP_ZTS !== true || ZEND_THREAD_SAFE !== true) {
    fwrite(STDERR, "expected a thread-safe/ZTS PHP build\n");
    exit(3);
}

$runtimeRaw = getenv('COBBLESTONE_RUNTIME_ID');
$runtimeId = $runtimeRaw === false ? 0 : (int) $runtimeRaw;
if ($runtimeId <= 0 || (string) $runtimeId !== $runtimeRaw) {
    fwrite(STDERR, "invalid COBBLESTONE_RUNTIME_ID\n");
    exit(4);
}

gc_enable();
printf("BOOT\t%d\t%d\t1\n", $runtimeId, getmypid());
fflush(STDOUT);

while (($line = fgets(STDIN)) !== false) {
    $fields = explode("\t", rtrim($line, "\r\n"));
    $kind = $fields[0] ?? '';

    if ($kind === 'MSG' && count($fields) === 5) {
        printf("MSG\t%s\t%s\t%s\t%s\n", $fields[1], $fields[2], $fields[3], $fields[4]);
        continue;
    }

    if ($kind === 'GC' && count($fields) === 2) {
        $left = new stdClass();
        $right = new stdClass();
        $left->peer = $right;
        $right->peer = $left;
        unset($left, $right);
        $cycles = gc_collect_cycles();
        printf("GC\t%s\t%d\t%d\n", $fields[1], $cycles, memory_get_usage(true));
        continue;
    }

    if ($kind === 'FLUSH' && count($fields) === 2) {
        $countRaw = $fields[1];
        $count = (int) $countRaw;
        if ($count <= 0 || (string) $count !== $countRaw) {
            fwrite(STDERR, "invalid runtime batch size\n");
            exit(5);
        }
        printf("FLUSHED\t%d\n", $count);
        fflush(STDOUT);
        continue;
    }

    if ($kind === 'STOP' && count($fields) === 2 && $fields[1] === '0') {
        fwrite(STDOUT, "STOPPED\t0\n");
        fflush(STDOUT);
        exit(0);
    }

    fwrite(STDERR, sprintf("invalid runtime command: %s\n", rtrim($line, "\r\n")));
    exit(5);
}

exit(0);
