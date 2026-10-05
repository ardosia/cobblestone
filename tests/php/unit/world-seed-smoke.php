<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\World\Generator\WorldSeed;

function worldSeedExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$default = 0x1234_5678;

worldSeedExpect(WorldSeed::fromTargetInput('', $default) === $default, 'empty seed did not use default');
worldSeedExpect(WorldSeed::fromTargetInput('0', $default) === $default, 'single-character 0 did not use default');
worldSeedExpect(WorldSeed::fromTargetInput('1', $default) === $default, 'single-character 1 did not use default');
worldSeedExpect(WorldSeed::fromTargetInput(' 0 ', $default) === 0, 'padded 0 did not parse like target');
worldSeedExpect(WorldSeed::fromTargetInput('-1', $default) === -1, 'special -1 seed mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('-2147483648', $default) === -2147483648, 'i32 min mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('2147483647', $default) === 2147483647, 'i32 max mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('+42', $default) === 42, 'plus-prefixed numeric mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('001337', $default) === 1337, 'zero-padded numeric mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('123abc', $default) === 123, 'sscanf numeric-prefix quirk mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('abc', $default) === 96354, 'ASCII text hash mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('-1foo', $default) === 43119778, '-1 suffix hash quirk mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('Cobblestone', $default) === 1847455552, 'named seed hash mismatch');
worldSeedExpect(WorldSeed::fromTargetInput('é', $default) === -1978, 'UTF-8 signed-byte hash mismatch');

try {
    WorldSeed::fromTargetInput('2147483648', $default);
    throw new RuntimeException('out-of-range numeric seed was accepted');
} catch (InvalidArgumentException) {
}

fwrite(STDOUT, "world-seed-smoke: passed\n");
