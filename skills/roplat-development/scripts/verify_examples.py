#!/usr/bin/env python3
"""Build, run and lint the skill's System examples in a temporary directory.

Run control_layers and robot_apps with their published registry dependencies
and --locked. No source repository is needed for the default validation.
robot_behavior uses nightly features; robot_apps declares that in its
rust-toolchain.toml, which applies because Cargo runs inside the copied project.

--patch NAME=PATH builds against a local source of a crate instead of the
published one for maintainer diagnostics; this is not release validation. The pinned lockfile cannot
hold then, so --locked is dropped and the evidence records the patches.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

TRACKED = ("roplat", "robot_behavior")
EXAMPLES = ("control_layers", "robot_apps")


def toml_string(value):
    # JSON-quoted ASCII strings are valid TOML basic strings, and every
    # subprocess receives an argument list, so paths never become shell code.
    return json.dumps(value, ensure_ascii=True)


def resolved(project):
    lock = tomllib.loads((project / "Cargo.lock").read_text(encoding="utf-8"))
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
        lines += [f"{toml_string(crate)} = {{ path = {toml_string(path.as_posix())} }}" for crate, path in patches.items()]
        with (project / "Cargo.toml").open("a", encoding="utf-8") as manifest:
            manifest.write("\n".join(lines) + "\n")
    env = dict(os.environ, CARGO_TARGET_DIR=os.environ.get("CARGO_TARGET_DIR", str(work / "target")),
               BULLET_SKIP_ASSET_EXPORT="1", ROPLAT_SKIP_ASSET_EXPORT="1")
    common = [] if patches else ["--locked"]
    if offline:
        common.append("--offline")
    used_patches = []
    if patches:
        command = ["cargo", "metadata", "--format-version", "1", *common]
        metadata = subprocess.run(command, env=env, cwd=project, text=True,
                                  encoding="utf-8", errors="replace", capture_output=True)
        (work / f"{name}-metadata.log").write_text(metadata.stderr, encoding="utf-8")
        errors = []
        if metadata.returncode:
            errors.append(metadata.stderr)
        else:
            packages = json.loads(metadata.stdout)["packages"]
            for crate, path in patches.items():
                selected = [package for package in packages if package["name"] == crate]
                if not selected:
                    continue
                if any(package["source"] is not None or
                       Path(package["manifest_path"]).resolve() != path / "Cargo.toml"
                       for package in selected):
                    errors.append(f"{crate}: Cargo did not exclusively select requested source {path}")
                else:
                    used_patches.append(crate)
        if errors:
            print("\n".join(errors))
            return {"results": [{"example": name, "step": "source-selection", "command": command,
                                  "returncode": metadata.returncode or 1, "errors": errors}],
                    "used_patches": used_patches}
    toolchain = subprocess.run(["rustc", "-V"], cwd=project, text=True, encoding="utf-8", errors="replace", capture_output=True)
    results = []
    for step, command in (
        ("run", ["cargo", "run", *common]),
        ("clippy", ["cargo", "clippy", *common, "--", "-D", "warnings"]),
    ):
        result = subprocess.run(command, env=env, cwd=project, text=True, encoding="utf-8", errors="replace", capture_output=True)
        (work / f"{name}-{step}.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        print(result.stdout, end="")
        print(result.stderr, end="")
        results.append({"example": name, "step": step, "command": command, "returncode": result.returncode})
        if result.returncode:
            break
    summary = {"toolchain": toolchain.stdout.strip(), "results": results,
               "used_patches": used_patches,
               "unused_patches": sorted(set(patches) - set(used_patches))}
    if (project / "Cargo.lock").is_file():
        summary["resolved"] = resolved(project)
    return summary


def main():
    # Cargo emits UTF-8 diagnostics even on Windows hosts with a legacy locale.
    # Keep complete UTF-8 logs and make unrepresentable console characters safe.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--offline", action="store_true", help="use only cached Cargo dependencies")
    parser.add_argument("--profile", choices=("published",), default="published",
                        help="validate the published examples (default)")
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
    for crate, source in patches.items():
        manifest = source / "Cargo.toml"
        if not manifest.is_file():
            parser.error(f"missing source crate manifest: {manifest}")
        package = tomllib.loads(manifest.read_text(encoding="utf-8")).get("package", {})
        if package.get("name") != crate:
            parser.error(f"patch {crate!r} points to {package.get('name')!r}, not the expected crate: {source}")

    assets = Path(__file__).resolve().parents[1] / "assets"
    examples = list(EXAMPLES)
    unknown = set(args.only) - set(examples)
    if unknown:
        parser.error(f"unknown example(s) {sorted(unknown)}; available: {examples}")
    if args.only:
        examples = [name for name in examples if name in args.only]

    work = Path(tempfile.mkdtemp(prefix="roplat-skill-example-"))
    print(f"Validation workspace: {work}", flush=True)
    sources = {}
    for crate, path in patches.items():
        head = subprocess.run(["git", "-C", str(path), "rev-parse", "HEAD"], text=True, encoding="utf-8", errors="replace", capture_output=True)
        status = subprocess.run(["git", "-C", str(path), "status", "--porcelain"], text=True, encoding="utf-8", errors="replace", capture_output=True)
        sources[crate] = {"path": str(path), "commit": head.stdout.strip() if head.returncode == 0 else None,
                          "status": status.stdout.splitlines() if status.returncode == 0 else None}
    evidence = {"profile": args.profile, "patches": {crate: str(path) for crate, path in patches.items()},
                "sources": sources, "examples": {}}
    failed = False
    for name in examples:
        summary = validate(name, assets, work, patches, args.offline)
        evidence["examples"][name] = summary
        if summary["results"][-1]["returncode"]:
            failed = True
            break
    used = {crate for summary in evidence["examples"].values() for crate in summary.get("used_patches", [])}
    required = {item.partition("=")[0] for item in args.patch}
    unused = sorted(required - used)
    if not failed and unused:
        evidence["unvalidated_patches"] = unused
        print(f"Requested source patches were not exercised by the selected examples: {unused}")
        failed = True
    (work / "validation.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(("Validation failed; inspect evidence: " if failed else "Validation passed; evidence: ") + str(work))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
