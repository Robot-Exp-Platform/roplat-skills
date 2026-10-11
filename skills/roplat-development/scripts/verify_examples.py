#!/usr/bin/env python3
"""Build and run the skill's System-macro examples against local checkouts.

control_layers needs only the roplat core. robot_apps also needs drives'
robot_behavior; it is skipped with a notice when --drives is not given.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def toml_path(path):
    # JSON-quoted ASCII strings with forward slashes are valid TOML path strings
    # and never become shell code: every subprocess receives an argument list.
    return json.dumps(path.as_posix(), ensure_ascii=True)


def validate(name, skill, work, placeholders, offline):
    project = work / name
    shutil.copytree(skill / "assets" / name, project)
    manifest = (project / "Cargo.toml.in").read_text()
    for key, value in placeholders.items():
        manifest = manifest.replace(key, value)
    (project / "Cargo.toml").write_text(manifest)
    env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
    common = ["--manifest-path", str(project / "Cargo.toml"), "--locked"]
    if offline:
        common.append("--offline")
    results = []
    for step, command in (
        ("run", ["cargo", "run", *common]),
        ("clippy", ["cargo", "clippy", *common, "--", "-D", "warnings"]),
    ):
        result = subprocess.run(command, env=env, cwd=work, text=True, capture_output=True)
        (work / f"{name}-{step}.log").write_text(result.stdout + result.stderr)
        print(result.stdout, end="")
        print(result.stderr, end="")
        results.append({"example": name, "command": command, "returncode": result.returncode})
        if result.returncode:
            break
    return results


def revision(root):
    head = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, capture_output=True)
    status = subprocess.run(["git", "-C", str(root), "status", "--porcelain"], text=True, capture_output=True)
    return {"root": str(root), "commit": head.stdout.strip(), "dirty": bool(status.stdout.strip())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roplat", required=True, type=Path, help="roplat repository root")
    parser.add_argument("--drives", type=Path, help="drives repository root (enables robot_apps)")
    parser.add_argument("--offline", action="store_true", help="use only cached Cargo dependencies")
    args = parser.parse_args()
    root = args.roplat.expanduser().resolve()
    crate = root / "roplat"
    if not (crate / "Cargo.toml").is_file():
        parser.error("--roplat must point to the repository containing roplat/Cargo.toml")
    placeholders = {"@ROPLAT_CRATE_PATH@": toml_path(crate)}
    examples = ["control_layers"]
    sources = {"core": revision(root)}
    if args.drives:
        drives = args.drives.expanduser().resolve()
        behavior = drives / "robot_behavior"
        if not (behavior / "Cargo.toml").is_file():
            parser.error("--drives must point to the repository containing robot_behavior/")
        placeholders["@ROBOT_BEHAVIOR_PATH@"] = toml_path(behavior)
        examples.append("robot_apps")
        sources["drives"] = revision(drives)
    else:
        print("Skipping robot_apps: pass --drives to validate it.")

    skill = Path(__file__).resolve().parents[1]
    work = Path(tempfile.mkdtemp(prefix="roplat-skill-example-"))
    results = []
    for name in examples:
        results += validate(name, skill, work, placeholders, args.offline)
        if results[-1]["returncode"]:
            print("Validation failed; inspect evidence:", work)
            return results[-1]["returncode"]
    (work / "validation.json").write_text(
        json.dumps({"sources": sources, "results": results}, indent=2) + "\n"
    )
    print("Validation passed; evidence:", work)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
