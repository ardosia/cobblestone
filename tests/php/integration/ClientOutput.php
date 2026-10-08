<?php

declare(strict_types=1);

/**
 * Child output for live integration tests. PHP proc_open() pipes cannot be read nonblocking on
 * Windows; a pipe read stalls the PHP-owned server tick loop until the child exits.
 * Regular files can be read up to their current length without waiting for future child output.
 */
final class ClientOutput
{
    private readonly string $stdoutPath;
    private readonly string $stderrPath;
    private int $stdoutOffset = 0;
    private int $stderrOffset = 0;

    public function __construct()
    {
        $stdout = tempnam(sys_get_temp_dir(), 'cb-stdout-');
        if ($stdout === false) {
            throw new RuntimeException('cannot allocate client stdout file');
        }
        $stderr = tempnam(sys_get_temp_dir(), 'cb-stderr-');
        if ($stderr === false) {
            unlink($stdout);
            throw new RuntimeException('cannot allocate client stderr file');
        }
        $this->stdoutPath = $stdout;
        $this->stderrPath = $stderr;
    }

    /** @return array<int, array{string, string}|array{string, string, string}> */
    public function descriptors(): array
    {
        return [
            0 => ['pipe', 'r'],
            1 => ['file', $this->stdoutPath, 'w'],
            2 => ['file', $this->stderrPath, 'w'],
        ];
    }

    public function readStdout(): string
    {
        return $this->readNew($this->stdoutPath, $this->stdoutOffset);
    }

    public function readStderr(): string
    {
        return $this->readNew($this->stderrPath, $this->stderrOffset);
    }

    public function close(): void
    {
        @unlink($this->stdoutPath);
        @unlink($this->stderrPath);
    }

    public function __destruct()
    {
        $this->close();
    }

    private function readNew(string $path, int &$offset): string
    {
        clearstatcache(true, $path);
        $size = filesize($path);
        if ($size === false) {
            throw new RuntimeException("cannot stat client output {$path}");
        }
        if ($size <= $offset) {
            return '';
        }
        $chunk = file_get_contents($path, false, null, $offset, $size - $offset);
        if ($chunk === false) {
            throw new RuntimeException("cannot read client output {$path}");
        }
        $offset += strlen($chunk);
        return $chunk;
    }
}
