#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONVERGENCE = {
    "requirements", "specs", "implementation", "tests", "documentation",
    "decisions", "tasks", "evidence", "repository",
}
CHANGE_STATUSES = {"planned", "active", "blocked", "implemented", "verified", "done", "abandoned"}
TASK_STATUSES = {"planned", "active", "blocked", "implemented", "verified", "done", "cancelled"}
CONVERGENCE_STATUSES = {"not_applicable", "pending", "aligned", "gap", "blocked"}
WORK_SIZES = {"S0", "S1", "S2", "S3"}
WORKFLOWS = {"feature", "bug", "research", "reverse_engineering", "operation", "recovery"}
OID = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")
REQ = re.compile(r"^##\s+([A-Z][A-Z0-9_-]+-\d+)\s+—")


class ValidationError(RuntimeError):
    pass


def load_toml(path: Path) -> dict:
    with path.open("rb") as f:
        return tomllib.load(f)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def check_dag(name: str, graph: dict[str, list[str]]) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visited:
            return
        if node in visiting:
            raise ValidationError(f"{name}: dependency cycle at {node}")
        visiting.add(node)
        for dep in graph[node]:
            visit(dep)
        visiting.remove(node)
        visited.add(node)

    for node in graph:
        visit(node)


def validate_project() -> None:
    path = ROOT / ".agent/project.toml"
    require(path.is_file(), "missing .agent/project.toml")
    p = load_toml(path)
    require(p.get("schema") == 1, ".agent/project.toml: schema must be 1")
    require(p.get("id") == "cobblestone", ".agent/project.toml: id must be cobblestone")
    require("ardosia/cobblestone" in p.get("repositories", []), ".agent/project.toml: canonical repository missing")
    require(p.get("default_branch") == "main", ".agent/project.toml: default branch must be main")


def validate_fixed_target() -> None:
    required = ["0.15.10", "protocol 84", "protocol 8", "PHP 8.5 ZTS"]
    texts = [
        (ROOT / "README.md").read_text(encoding="utf-8"),
        (ROOT / ".agent/specs/foundation.md").read_text(encoding="utf-8"),
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


def validate_specs() -> None:
    ids: dict[str, Path] = {}
    specs = sorted((ROOT / ".agent/specs").glob("*.md"))
    require(bool(specs), ".agent/specs: no spec files")
    for path in specs:
        for line in path.read_text(encoding="utf-8").splitlines():
            m = REQ.match(line)
            if not m:
                continue
            rid = m.group(1)
            if rid in ids:
                raise ValidationError(f"duplicate requirement id {rid}: {ids[rid]} and {path}")
            ids[rid] = path
    require(bool(ids), ".agent/specs: no stable requirement IDs")


def validate_changes() -> None:
    root = ROOT / ".agent/changes"
    require(root.is_dir(), "missing .agent/changes")
    for change_dir in sorted(p for p in root.iterdir() if p.is_dir()):
        rel = change_dir.relative_to(ROOT)
        cpath = change_dir / "change.toml"
        require(cpath.is_file(), f"{rel}: missing change.toml")
        c = load_toml(cpath)
        require(c.get("schema") == 1, f"{rel}/change.toml: schema must be 1")
        require(c.get("id") == change_dir.name, f"{rel}/change.toml: id must match directory")
        require(c.get("size") in WORK_SIZES, f"{rel}/change.toml: invalid size")
        require(c.get("status") in CHANGE_STATUSES, f"{rel}/change.toml: invalid status")
        require(c.get("workflow") in WORKFLOWS, f"{rel}/change.toml: invalid workflow")
        require(OID.fullmatch(c.get("control_revision", "")) is not None, f"{rel}/change.toml: invalid control_revision")
        require(OID.fullmatch(c.get("base_revision", "")) is not None, f"{rel}/change.toml: invalid base_revision")
        acceptance = c.get("acceptance")
        require(isinstance(acceptance, list) and acceptance and all(isinstance(x, str) and x.strip() for x in acceptance), f"{rel}/change.toml: invalid acceptance")
        conv = c.get("convergence", {})
        require(set(conv) == CONVERGENCE, f"{rel}/change.toml: convergence keys mismatch")
        require(all(v in CONVERGENCE_STATUSES for v in conv.values()), f"{rel}/change.toml: invalid convergence status")

        tpath = change_dir / "tasks.toml"
        if tpath.is_file():
            tdoc = load_toml(tpath)
            require(tdoc.get("schema") == 1 and tdoc.get("change") == change_dir.name, f"{rel}/tasks.toml: identity mismatch")
            tasks = tdoc.get("tasks", [])
            ids = [t.get("id") for t in tasks if isinstance(t, dict)]
            require(len(ids) == len(set(ids)) and all(ids), f"{rel}/tasks.toml: task ids must be unique/non-empty")
            idset = set(ids)
            graph: dict[str, list[str]] = {}
            for task in tasks:
                tid = task.get("id")
                require(task.get("status") in TASK_STATUSES, f"{rel}/tasks.toml: {tid} invalid status")
                deps = task.get("depends_on", [])
                require(isinstance(deps, list) and all(d in idset for d in deps), f"{rel}/tasks.toml: {tid} invalid dependency")
                require(isinstance(task.get("acceptance"), str) and task["acceptance"].strip(), f"{rel}/tasks.toml: {tid} missing acceptance")
                graph[tid] = deps
            check_dag(str(rel / "tasks.toml"), graph)

        epath = change_dir / "evidence.toml"
        if epath.is_file():
            edoc = load_toml(epath)
            require(edoc.get("schema") == 1 and edoc.get("change") == change_dir.name, f"{rel}/evidence.toml: identity mismatch")
            evidence = edoc.get("evidence", [])
            ids = [e.get("id") for e in evidence if isinstance(e, dict)]
            require(len(ids) == len(set(ids)) and all(ids), f"{rel}/evidence.toml: evidence ids must be unique/non-empty")
            for item in evidence:
                for key in ("id", "kind", "status", "claim", "source", "result"):
                    require(isinstance(item.get(key), str) and item[key].strip(), f"{rel}/evidence.toml: evidence missing {key}")


def validate_metadata() -> None:
    validate_project()
    validate_fixed_target()
    validate_ci_workflow()
    validate_specs()
    validate_changes()
    print("metadata: passed")


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def validate_rust() -> None:
    cargo_toml = ROOT / "Cargo.toml"
    if not cargo_toml.is_file():
        print("rust: workspace not present yet; C002 has not started")
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
        print("bench: workspace not present yet; C002 has not started")
        return
    require(shutil.which("cargo") is not None, "cargo is required for the native benchmark")
    run(["cargo", "bench", "-p", "cobblestone-core", "--bench", "core"])
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
