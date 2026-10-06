#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class ValidationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def validate_rust() -> None:
    cargo_toml = ROOT / "Cargo.toml"
    require(cargo_toml.is_file(), "Cargo.toml is required for Rust validation")
    require(shutil.which("cargo") is not None, "cargo is required once Cargo.toml exists")
    run(["cargo", "fmt", "--check"])
    run(["cargo", "check", "--locked", "--workspace", "--all-targets"])
    run(["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"])
    run(["cargo", "test", "--locked", "--workspace", "--lib", "--tests"])
    print("rust: passed")

def benchmark_rust() -> None:
    cargo_toml = ROOT / "Cargo.toml"
    if not cargo_toml.is_file():
        print("bench: workspace not present")
        return
    require(shutil.which("cargo") is not None, "cargo is required for the native benchmark")
    run(["cargo", "bench", "-p", "cobblestone-runtime", "--bench", "runtime"])
    run(["cargo", "bench", "-p", "cobblestone-world", "--bench", "world"])
    print("bench: completed")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("rust", "bench", "all"), nargs="?", default="all")
    args = parser.parse_args()
    try:
        if args.mode in {"rust", "all"}:
            validate_rust()
        if args.mode == "bench":
            benchmark_rust()
    except (ValidationError, subprocess.CalledProcessError) as exc:
        print(f"validation failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
