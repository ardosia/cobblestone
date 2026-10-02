<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Command\Command;
use Cobblestone\Command\CommandDefinitionException;
use Cobblestone\Command\CommandParseException;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Command\CommandRequirementFailed;
use function Cobblestone\Command\{boolean, enumArg, greedyString, integer, literal, word};

enum CommandSmokeMode: string
{
    case Survival = 'survival';
    case Creative = 'creative';
}

function commandExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$commands = new CommandRegistry();
$binding = $commands->register(
    literal('world', aliases: ['w'])->then(
        literal('time')->then(
            literal('get')->executes(
                static fn (Command $command): string => $command->input(),
            ),
            literal('set')->then(
                integer('time', min: 0, max: 24_000)->executes(
                    static fn (int $time): int => $time,
                ),
            ),
        ),
        literal('mode')->then(
            enumArg('mode', CommandSmokeMode::class)->executes(
                static fn (CommandSmokeMode $mode): string => $mode->value,
            ),
        ),
        literal('flag')->then(
            boolean('enabled')->executes(
                static fn (bool $enabled): bool => $enabled,
            ),
        ),
        literal('say')->then(
            greedyString('message')->executes(
                static fn (string $message): string => $message,
            ),
        ),
        literal('optional')
            ->executes(static fn (): string => 'base')
            ->then(
                word('name')->executes(
                    static fn (string $name): string => $name,
                ),
            ),
        literal('ban')->then(
            literal('list')->executes(static fn (): string => 'literal'),
            word('player')->executes(
                static fn (string $player): string => 'player:' . $player,
            ),
        ),
        literal('admin')
            ->requires(static fn (Command $command): bool => $command->source() === 'operator')
            ->executes(static fn (): string => 'admin'),
    ),
);

commandExpect($commands->execute('/world time set 42') === 42, 'integer argument did not inject');
commandExpect($commands->execute('w mode creative') === 'creative', 'enum argument did not resolve');
commandExpect($commands->execute('world flag true') === true, 'boolean argument did not resolve');
commandExpect($commands->execute('world say hello there') === 'hello there', 'greedy argument lost text');
commandExpect($commands->execute('world optional') === 'base', 'executable parent did not model optional tail');
commandExpect($commands->execute('world optional steve') === 'steve', 'optional child did not execute');
commandExpect($commands->execute('world ban list') === 'literal', 'literal did not win over argument');
commandExpect($commands->execute('world ban steve') === 'player:steve', 'argument fallback did not execute');
commandExpect($commands->suggest('wo') === ['world'], 'root suggestion mismatch');
commandExpect($commands->suggest('world time ') === ['get', 'set'], 'literal child suggestions mismatch');
commandExpect($commands->suggest('world mode c') === ['creative'], 'enum suggestions mismatch');

try {
    $commands->execute('world admin', 'guest');
    throw new RuntimeException('command requirement did not reject guest source');
} catch (CommandRequirementFailed) {
}
commandExpect($commands->execute('world admin', 'operator') === 'admin', 'command requirement rejected operator');

try {
    $commands->register(
        literal('broken')->then(
            integer('count')->executes(
                static fn (string $count): string => $count,
            ),
        ),
    );
    throw new RuntimeException('handler type mismatch was not rejected');
} catch (CommandDefinitionException) {
}

commandExpect($binding->cancel(), 'command binding did not cancel');
try {
    $commands->execute('world time get');
    throw new RuntimeException('cancelled command root remained registered');
} catch (CommandParseException) {
}

fwrite(STDOUT, "command-smoke: passed\n");
