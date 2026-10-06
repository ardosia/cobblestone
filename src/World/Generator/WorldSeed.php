<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

/**
 * Fixed-target MCPE 0.15.10 seed-box conversion.
 *
 * The target stores RandomSeed as raw 32-bit bits. Cobblestone carries the same bits through its
 * signed PHP/native seed API, so values with bit 31 set are represented as negative integers.
 */
final class WorldSeed
{
    private const U32_MODULUS = 4_294_967_296;
    private const I32_MAX = 2_147_483_647;
    private const I32_MIN = -2_147_483_648;

    private function __construct() {}

    public static function random(): int
    {
        return self::signedFromU32(random_int(0, 0xffff_ffff));
    }

    /**
     * Mirrors LevelSettings::parseSeedString(input, defaultSeed) for target-stable inputs.
     *
     * Numeric prefixes are accepted like target sscanf("%d"). Numeric values outside the signed
     * 32-bit conversion domain are rejected because the target C runtime's overflow behavior is
     * not a portable world-format contract.
     */
    public static function fromTargetInput(string $input, int $defaultSeed): int
    {
        self::assertSigned32($defaultSeed);

        if (strlen($input) < 2) {
            return $defaultSeed;
        }

        $seed = trim($input, " \t\n\r");
        if ($seed === '') {
            return $defaultSeed;
        }

        if (preg_match('/^[+-]?\d+/', $seed, $match) === 1) {
            $prefix = $match[0];
            if (self::decimalFitsSigned32($prefix)) {
                $numeric = (int) $prefix;
                if ($numeric !== -1 || $seed === '-1') {
                    return $numeric;
                }
            } elseif ($seed === $prefix) {
                throw new \InvalidArgumentException(
                    'MCPE 0.15.10 numeric seed input outside signed 32-bit range is unsupported',
                );
            }
        }

        return self::signedFromU32(self::targetHashCode($seed));
    }

    public static function targetHashCode(string $seed): int
    {
        $hash = 0;
        $length = strlen($seed);
        for ($index = 0; $index < $length; ++$index) {
            $byte = ord($seed[$index]);
            $signedByte = $byte >= 0x80 ? $byte - 0x100 : $byte;
            $hash = (($hash * 31) + $signedByte) & 0xffff_ffff;
        }

        return $hash;
    }

    private static function decimalFitsSigned32(string $value): bool
    {
        $negative = str_starts_with($value, '-');
        $digits = ltrim($value, '+-');
        $digits = ltrim($digits, '0');
        if ($digits === '') {
            return true;
        }

        $limit = $negative ? '2147483648' : '2147483647';
        $length = strlen($digits);

        return $length < strlen($limit)
            || ($length === strlen($limit) && strcmp($digits, $limit) <= 0);
    }

    private static function signedFromU32(int $value): int
    {
        return $value > self::I32_MAX ? $value - self::U32_MODULUS : $value;
    }

    private static function assertSigned32(int $seed): void
    {
        if ($seed < self::I32_MIN || $seed > self::I32_MAX) {
            throw new \InvalidArgumentException('default world seed must fit signed 32 bits');
        }
    }
}
