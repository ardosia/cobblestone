<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

/** @internal */
final readonly class Protocol84BootstrapResult
{
    public const LOGIN_ACCEPTED = 'login-accepted';
    public const SPAWNED = 'spawned';
    public const GAMEPLAY = 'gameplay';

    private function __construct(
        public string $kind,
        public ?int $requestedRadius = null,
    ) {
    }

    public static function loginAccepted(): self
    {
        return new self(self::LOGIN_ACCEPTED);
    }

    public static function spawned(int $requestedRadius): self
    {
        return new self(self::SPAWNED, $requestedRadius);
    }

    public static function gameplay(): self
    {
        return new self(self::GAMEPLAY);
    }
}
