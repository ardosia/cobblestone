<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\World;
use UnexpectedValueException;

/** @internal */
final class SessionGameplay
{
    private const MOVE_PLAYER_PACKET = 0x10;
    private const VIEW_DELTA_HEADER_BYTES = 20;
    private const VIEW_DELTA_ENTRY_BYTES = 8;
    private const MAX_VIEW_DELTA_ENTRIES = 4096;
    private const VIEW_SEND_BACKPRESSURED = 0;
    private const VIEW_SEND_SENT = 1;
    private const VIEW_SEND_GONE = 2;

    /** @var array<int, ChunkViewPreparation> */
    private array $preparations = [];

    public function __construct(
        private readonly Runtime $sessions,
        private readonly World $world,
    ) {
    }

    public function spawned(int $sessionId): void
    {
        $this->clearPreparation($sessionId);

        $spawn = $this->world->spawn();
        $this->sessions->initializePlayerPosition(
            $sessionId,
            $spawn->x,
            $spawn->y,
            $spawn->z,
        );
    }

    public function disconnected(int $sessionId): void
    {
        $this->clearPreparation($sessionId);
    }

    public function stop(): void
    {
        foreach (array_keys($this->preparations) as $sessionId) {
            $this->clearPreparation($sessionId);
        }
    }

    public function tick(): void
    {
        foreach ($this->preparations as $sessionId => $preparation) {
            $this->advancePreparation($sessionId, $preparation);
        }
    }

    public function handle(Packet $packet): void
    {
        if ($packet->packetId !== self::MOVE_PLAYER_PACKET) {
            return;
        }

        $projection = $this->sessions->trackPlayerMovement($packet->sessionId, $packet->body);
        $next = $this->decodePreparation($projection);
        if ($next === null) {
            $this->clearPreparation($packet->sessionId);
            return;
        }

        $current = $this->preparations[$packet->sessionId] ?? null;
        if ($current !== null && $current->sameTransition($next->fromCenter, $next->toCenter)) {
            $current->prepare($this->world);
            return;
        }

        $this->clearPreparation($packet->sessionId);
        $this->preparations[$packet->sessionId] = $next;
        $next->prepare($this->world);
    }

    private function advancePreparation(int $sessionId, ChunkViewPreparation $preparation): void
    {
        if ($preparation->sent() || !$preparation->prepare($this->world)) {
            return;
        }

        $status = $this->sessions->sendPreparedViewChunks(
            $sessionId,
            $preparation->fromCenter->x,
            $preparation->fromCenter->z,
            $preparation->toCenter->x,
            $preparation->toCenter->z,
        );

        if ($status === self::VIEW_SEND_BACKPRESSURED) {
            return;
        }
        if ($status === self::VIEW_SEND_SENT) {
            $preparation->markSent();
            return;
        }
        if ($status === self::VIEW_SEND_GONE) {
            $this->clearPreparation($sessionId);
            return;
        }

        throw new UnexpectedValueException('native prepared view send returned an invalid status');
    }

    private function clearPreparation(int $sessionId): void
    {
        $preparation = $this->preparations[$sessionId] ?? null;
        if ($preparation === null) {
            return;
        }

        unset($this->preparations[$sessionId]);
        $preparation->release();
    }

    private function decodePreparation(string $projection): ?ChunkViewPreparation
    {
        if ($projection === '') {
            return null;
        }
        if (strlen($projection) < self::VIEW_DELTA_HEADER_BYTES) {
            throw new UnexpectedValueException('native chunk view delta header is truncated');
        }

        $header = unpack(
            'Vfrom_x/Vfrom_z/Vto_x/Vto_z/Vcount',
            substr($projection, 0, self::VIEW_DELTA_HEADER_BYTES),
        );
        if (!is_array($header)) {
            throw new UnexpectedValueException('native chunk view delta header is invalid');
        }

        $count = $header['count'];
        if ($count > self::MAX_VIEW_DELTA_ENTRIES) {
            throw new UnexpectedValueException('native chunk view delta entry count is too large');
        }
        $expected = self::VIEW_DELTA_HEADER_BYTES + ($count * self::VIEW_DELTA_ENTRY_BYTES);
        if (strlen($projection) !== $expected) {
            throw new UnexpectedValueException('native chunk view delta width mismatch');
        }

        $entering = [];
        $offset = self::VIEW_DELTA_HEADER_BYTES;
        for ($index = 0; $index < $count; ++$index) {
            $coordinates = unpack('Vx/Vz', substr($projection, $offset, self::VIEW_DELTA_ENTRY_BYTES));
            if (!is_array($coordinates)) {
                throw new UnexpectedValueException('native chunk view delta entry is invalid');
            }
            $entering[] = new ChunkPos(
                self::signedInt32($coordinates['x']),
                self::signedInt32($coordinates['z']),
            );
            $offset += self::VIEW_DELTA_ENTRY_BYTES;
        }

        return new ChunkViewPreparation(
            new ChunkPos(
                self::signedInt32($header['from_x']),
                self::signedInt32($header['from_z']),
            ),
            new ChunkPos(
                self::signedInt32($header['to_x']),
                self::signedInt32($header['to_z']),
            ),
            $entering,
        );
    }

    private static function signedInt32(int $value): int
    {
        return $value >= 0x80000000 ? $value - 0x100000000 : $value;
    }
}
