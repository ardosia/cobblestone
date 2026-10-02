<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\LightUpdate;
use Cobblestone\World\World;
use InvalidArgumentException;
use SplQueue;

/**
 * Owner-runtime queue/control plane for the low-level propagation substrate.
 *
 * Queue entries are coalesced while pending. An update may be re-enqueued after it has been
 * processed if later light changes make it stale again.
 */
final readonly class LightEngine
{
    public function __construct(
        private LightPropagator $propagator,
        private BlockLightCatalog $catalog,
        private int $defaultBudget = 100_000,
    ) {
        if ($defaultBudget <= 0) {
            throw new InvalidArgumentException('light update budget must be positive');
        }
    }

    public static function fixedTarget(): self
    {
        $catalog = new BlockLightCatalog();

        return new self(new LightPropagator($catalog), $catalog);
    }

    public function apply(
        World $world,
        LightUpdate $initial,
        ?int $budget = null,
    ): LightPropagationResult {
        $budget ??= $this->defaultBudget;
        if ($budget <= 0) {
            throw new InvalidArgumentException('light update budget must be positive');
        }

        $access = new WorldLightAccess($world, $this->catalog);
        /** @var SplQueue<LightUpdate> $queue */
        $queue = new SplQueue();
        $pending = [];

        $submit = static function (LightUpdate $update) use ($queue, &$pending): void {
            $key = self::updateKey($update);
            if (isset($pending[$key])) {
                return;
            }

            $pending[$key] = true;
            $queue->enqueue($update);
        };

        $submit($initial);
        $processed = 0;
        while (!$queue->isEmpty()) {
            if ($processed >= $budget) {
                throw new LightPropagationException(
                    "light propagation exceeded {$budget} queued updates",
                );
            }

            /** @var LightUpdate $update */
            $update = $queue->dequeue();
            unset($pending[self::updateKey($update)]);
            ++$processed;
            $this->propagator->apply($access, $update, $submit);
        }

        return new LightPropagationResult($processed, $access->commit());
    }

    private static function updateKey(LightUpdate $update): string
    {
        return $update->layer->name
            . ':' . $update->min->x
            . ':' . $update->min->y
            . ':' . $update->min->z
            . ':' . $update->max->x
            . ':' . $update->max->y
            . ':' . $update->max->z;
    }
}
