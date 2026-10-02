<?php

declare(strict_types=1);

namespace Cobblestone\Command;

final class CommandReader
{
    private int $cursor;

    public function __construct(
        private readonly string $input,
        int $cursor = 0,
    ) {
        if ($cursor < 0 || $cursor > strlen($input)) {
            throw new \InvalidArgumentException('reader cursor is outside the input');
        }
        $this->cursor = $cursor;
    }

    public function copy(): self
    {
        return new self($this->input, $this->cursor);
    }

    public function cursor(): int
    {
        return $this->cursor;
    }

    public function atEnd(): bool
    {
        $cursor = $this->cursor;
        $length = strlen($this->input);
        while ($cursor < $length && ctype_space($this->input[$cursor])) {
            ++$cursor;
        }

        return $cursor >= $length;
    }

    public function skipWhitespace(): void
    {
        $length = strlen($this->input);
        while ($this->cursor < $length && ctype_space($this->input[$this->cursor])) {
            ++$this->cursor;
        }
    }

    public function readWord(): string
    {
        $this->skipWhitespace();
        $start = $this->cursor;
        $length = strlen($this->input);
        while ($this->cursor < $length && !ctype_space($this->input[$this->cursor])) {
            ++$this->cursor;
        }

        if ($this->cursor === $start) {
            throw new CommandParseException('expected command token', $start);
        }

        return substr($this->input, $start, $this->cursor - $start);
    }

    public function readString(): string
    {
        $this->skipWhitespace();
        if ($this->cursor >= strlen($this->input)) {
            throw new CommandParseException('expected string', $this->cursor);
        }
        if ($this->input[$this->cursor] !== '"') {
            return $this->readWord();
        }

        $start = $this->cursor++;
        $value = '';
        $escaped = false;
        $length = strlen($this->input);
        while ($this->cursor < $length) {
            $char = $this->input[$this->cursor++];
            if ($escaped) {
                if ($char !== '"' && $char !== '\\') {
                    $value .= '\\';
                }
                $value .= $char;
                $escaped = false;
                continue;
            }
            if ($char === '\\') {
                $escaped = true;
                continue;
            }
            if ($char === '"') {
                return $value;
            }
            $value .= $char;
        }

        throw new CommandParseException('unterminated quoted string', $start);
    }

    public function readRemaining(): string
    {
        $this->skipWhitespace();
        if ($this->cursor >= strlen($this->input)) {
            throw new CommandParseException('expected remaining command text', $this->cursor);
        }

        $value = substr($this->input, $this->cursor);
        $this->cursor = strlen($this->input);

        return $value;
    }
}
