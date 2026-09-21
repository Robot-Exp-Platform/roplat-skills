#!/usr/bin/env python3
"""Build finite System-macro examples against a selected local roplat checkout."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roplat", required=True, type=Path, help="roplat repository root")
    parser.add_argument("--offline", action="store_true", help="use only cached Cargo dependencies")
    args = parser.parse_args()
    root = args.roplat.expanduser().resolve()
    crate = root / "roplat"
    if not (crate / "Cargo.toml").is_file():
        parser.error("--roplat must point to the repository containing roplat/Cargo.toml")
    evaluation = Path(__file__).resolve().parent / "results" / "20260920" / "independent-use"
    work = Path(tempfile.mkdtemp(prefix="roplat-skill-evaluation-"))
    project = work / "independent-use"
    shutil.copytree(evaluation, project)
    template = (project / "Cargo.toml.in").read_text()
    # JSON quoted ASCII strings are valid for these TOML path strings and do not
    # become shell code: every subprocess below receives a list of arguments.
    manifest = template.replace("@ROPLAT_CRATE_PATH@", json.dumps(str(crate), ensure_ascii=True))
    (project / "Cargo.toml").write_text(manifest)
    env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
    common = ["--manifest-path", str(project / "Cargo.toml"), "--locked"]
    if args.offline:
        common.append("--offline")
    commands = [["cargo", "run", *common], ["cargo", "clippy", *common, "--", "-D", "warnings"]]
    results = []
    for command in commands:
        result = subprocess.run(command, env=env, cwd=work, text=True, capture_output=True)
        name = "run" if command[1] == "run" else "clippy"
        (work / (name + ".log")).write_text(result.stdout + result.stderr)
        print(result.stdout, end="")
        print(result.stderr, end="")
        results.append({"command": command, "returncode": result.returncode})
        if result.returncode:
            print("Validation failed; inspect evidence:", work)
            return result.returncode
    revision = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, capture_output=True)
    status = subprocess.run(["git", "-C", str(root), "status", "--porcelain"], text=True, capture_output=True)
    (work / "validation.json").write_text(json.dumps({
        "core_root": str(root), "core_commit": revision.stdout.strip(),
        "core_dirty": bool(status.stdout.strip()), "results": results,
    }, indent=2) + "\n")
    print("Validation passed; evidence:", work)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
