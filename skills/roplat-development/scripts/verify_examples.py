#!/usr/bin/env python3
"""Build, run and lint the skill's System examples in a temporary directory.

Each directory under assets/ is a standalone Cargo project that depends on the
published roplat crate (robot_apps also on robot_behavior) and keeps its own
Cargo.lock, so the check runs with --locked and needs no other repository.
robot_behavior uses nightly features; robot_apps declares that in its
rust-toolchain.toml, which applies because Cargo runs inside the copied project.

--patch NAME=PATH builds against a local source of a crate instead of the
published one, for checking an unreleased version. The pinned lockfile cannot
hold then, so --locked is dropped and the evidence records the patches.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

TRACKED = ("roplat", "robot_behavior")


def toml_string(value):
    # JSON-quoted ASCII strings are valid TOML basic strings, and every
    # subprocess receives an argument list, so paths never become shell code.
    return json.dumps(value, ensure_ascii=True)


def resolved(project):
    lock = tomllib.loads((project / "Cargo.lock").read_text())
    return {
        package["name"]: {"version": package["version"], "source": package.get("source", "local path")}
        for package in lock.get("package", [])
        if package["name"] in TRACKED
    }


def validate(name, assets, work, patches, offline):
    project = work / name
    shutil.copytree(assets / name, project, ignore=shutil.ignore_patterns("target"))
    if patches:
        lines = ["", "[patch.crates-io]"]
        lines += [f"{crate} = {{ path = {toml_string(path.as_posix())} }}" for crate, path in patches.items()]
        with (project / "Cargo.toml").open("a") as manifest:
            manifest.write("\n".join(lines) + "\n")
    env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
    common = [] if patches else ["--locked"]
    if offline:
        common.append("--offline")
    toolchain = subprocess.run(["rustc", "-V"], cwd=project, text=True, capture_output=True)
    results = []
    for step, command in (
        ("run", ["cargo", "run", *common]),
        ("clippy", ["cargo", "clippy", *common, "--", "-D", "warnings"]),
    ):
        result = subprocess.run(command, env=env, cwd=project, text=True, capture_output=True)
        (work / f"{name}-{step}.log").write_text(result.stdout + result.stderr)
        print(result.stdout, end="")
        print(result.stderr, end="")
        results.append({"example": name, "step": step, "command": command, "returncode": result.returncode})
        if result.returncode:
            break
    summary = {"toolchain": toolchain.stdout.strip(), "results": results}
    if (project / "Cargo.lock").is_file():
        summary["resolved"] = resolved(project)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--offline", action="store_true", help="use only cached Cargo dependencies")
    parser.add_argument("--only", action="append", default=[], metavar="EXAMPLE", help="check only this example (repeatable)")
    parser.add_argument(
        "--patch",
        action="append",
        default=[],
        metavar="CRATE=PATH",
        help="build against a local crate source instead of crates.io (repeatable; drops --locked)",
    )
    args = parser.parse_args()

    patches = {}
    for item in args.patch:
        crate, separator, path = item.partition("=")
        source = Path(path).expanduser().resolve()
        if not separator or not crate or not (source / "Cargo.toml").is_file():
            parser.error(f"--patch expects CRATE=PATH to a directory with Cargo.toml, got {item!r}")
        patches[crate] = source

    assets = Path(__file__).resolve().parents[1] / "assets"
    examples = sorted(path.name for path in assets.iterdir() if (path / "Cargo.toml").is_file())
    unknown = set(args.only) - set(examples)
    if unknown:
        parser.error(f"unknown example(s) {sorted(unknown)}; available: {examples}")
    if args.only:
        examples = [name for name in examples if name in args.only]

    work = Path(tempfile.mkdtemp(prefix="roplat-skill-example-"))
    evidence = {"patches": {crate: str(path) for crate, path in patches.items()}, "examples": {}}
    failed = False
    for name in examples:
        summary = validate(name, assets, work, patches, args.offline)
        evidence["examples"][name] = summary
        if summary["results"][-1]["returncode"]:
            failed = True
            break
    (work / "validation.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(("Validation failed; inspect evidence: " if failed else "Validation passed; evidence: ") + str(work))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
