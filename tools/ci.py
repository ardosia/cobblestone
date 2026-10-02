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


def validate_fixed_target() -> None:
    required = ["0.15.10", "protocol 84", "protocol 8", "PHP 8.5 ZTS"]
    texts = [
        (ROOT / "README.md").read_text(encoding="utf-8"),
        (ROOT / "docs/architecture/FOUNDATION.md").read_text(encoding="utf-8"),
    ]
    joined = "\n".join(texts)
    for token in required:
        require(token in joined, f"fixed-target marker missing: {token}")


def validate_ci_workflow() -> None:
    path = ROOT / ".github/workflows/ci.yml"
    require(path.is_file(), "missing .github/workflows/ci.yml")
    text = path.read_text(encoding="utf-8")
    require("\\${{" not in text, ".github/workflows/ci.yml: escaped GitHub Actions expression")
    require(
        text.count("runs-on: ${{ matrix.os }}") == 2,
        ".github/workflows/ci.yml: validate/php-zts matrix runner expressions must be intact",
    )


def validate_metadata() -> None:
    validate_fixed_target()
    validate_ci_workflow()
    print("metadata: passed")


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def validate_rust() -> None:
    cargo_toml = ROOT / "Cargo.toml"
    if not cargo_toml.is_file():
        print("rust: workspace not present")
        return
    require(shutil.which("cargo") is not None, "cargo is required once Cargo.toml exists")
    run(["cargo", "fmt", "--check"])
    run(["cargo", "check", "--workspace", "--all-targets"])
    run(["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])
    run(["cargo", "test", "--workspace", "--lib", "--tests"])
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
    parser.add_argument("mode", choices=("metadata", "rust", "bench", "all"), nargs="?", default="all")
    args = parser.parse_args()
    try:
        if args.mode in {"metadata", "all"}:
            validate_metadata()
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
