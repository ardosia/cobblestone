<?php

declare(strict_types=1);

namespace Cobblestone\Session\Internal;

final readonly class BootstrapUpdate
{
    public const LOGIN_ACCEPTED = 'login-accepted';
    public const CHUNKS_LOADING = 'chunks-loading';
    public const SPAWNED = 'spawned';
    public const GAMEPLAY = 'gameplay';

    private function __construct(
        public string $kind,
        public ?int $requestedRadius = null,
        public ?int $effectiveRadius = null,
        public int $chunksSent = 0,
        public int $encodedBytes = 0,
        public int $chunkEncodeNanos = 0,
    ) {}

    public static function loginAccepted(): self
    {
        return new self(self::LOGIN_ACCEPTED);
    }

    public static function chunksLoading(
        int $requestedRadius,
        int $effectiveRadius,
    ): self {
        return new self(
            self::CHUNKS_LOADING,
            $requestedRadius,
            $effectiveRadius,
        );
    }

    public static function spawned(
        int $requestedRadius,
        int $effectiveRadius,
        int $chunksSent,
        int $encodedBytes,
        int $chunkEncodeNanos,
    ): self {
        return new self(
            self::SPAWNED,
            $requestedRadius,
            $effectiveRadius,
            $chunksSent,
            $encodedBytes,
            $chunkEncodeNanos,
        );
    }

    public static function gameplay(): self
    {
        return new self(self::GAMEPLAY);
    }
}
