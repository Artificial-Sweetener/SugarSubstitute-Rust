# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Choose the focused Windows test selections for a lane push or an on-demand focused run.

Decision M-4: lanes run `cargo xtask check` plus their own area's tests; the full suite runs once on
the combined branch. A lane names its tests in LANE-TESTS.txt at the repository root, one entry per
line as `-p <package> [<filter>]`, optionally prefixed by `cargo test --locked`. Blank lines and
lines starting with # are ignored. Without that file, the tests of every workspace package the push
changed are selected. Entries can only add test selections: each names one workspace package and an
optional plain test-name filter, and nothing else is accepted.

Prints one JSON argument list per selection, each beginning with "test", for cargo to run.
"""
import argparse
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tomllib

LANE_FILE = "LANE-TESTS.txt"
WORKSPACE = PurePosixPath("Substitute")
# Components outside the workspace that Substitute compiles against, with their consuming sources.
COMPONENTS = WORKSPACE / "governance/monorepo/components.toml"
FILTER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z0-9_]+)*")
MAX_SELECTIONS = 32
EMPTY_REVISION = re.compile(r"0+")


class SelectionError(ValueError):
    """A lane test entry or request that would do more than select tests."""


def workspace_packages(root):
    """Map each workspace member's manifest directory, relative to the repository, to its name."""
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
         "--manifest-path", str(root / WORKSPACE / "Cargo.toml")],
        check=True, capture_output=True, text=True).stdout)
    workspace_root = Path(metadata["workspace_root"]).resolve()
    members = set(metadata["workspace_members"])
    packages = {}
    for package in metadata["packages"]:
        if package["id"] in members:
            directory = Path(package["manifest_path"]).resolve().parent.relative_to(workspace_root)
            packages[WORKSPACE / PurePosixPath(directory.as_posix())] = package["name"]
    return packages


def selection(package, test_filter, names):
    """Return one validated cargo test argument list."""
    if package not in names:
        raise SelectionError(f"{package!r} is not a workspace package")
    arguments = ["test", "--locked", "-p", package, "--all-features"]
    if test_filter:
        if len(test_filter) > 200 or not FILTER.fullmatch(test_filter):
            raise SelectionError(f"{test_filter!r} is not a plain test-name filter")
        arguments.append(test_filter)
    return arguments


def parse_entry(line, names):
    """Accept exactly `[cargo test --locked] -p <package> [<filter>]`."""
    words = line.split()
    if words[:3] == ["cargo", "test", "--locked"]:
        words = words[3:]
    if len(words) not in (2, 3) or words[0] != "-p":
        raise SelectionError(f"{line!r} is not '-p <package> [<filter>]'")
    return selection(words[1], words[2] if len(words) == 3 else None, names)


def lane_entries(text, names):
    selections = []
    for line in text.splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            selections.append(parse_entry(line, names))
    return selections


def owning_package(path, packages):
    """Return the workspace package whose manifest directory most closely contains path."""
    owner = None
    for directory, name in packages.items():
        if path.is_relative_to(directory) and (owner is None or len(directory.parts) > len(owner[0].parts)):
            owner = (directory, name)
    return owner[1] if owner else None


def component_consumers(text):
    """Map each external component root to the workspace-relative sources that consume it."""
    return {PurePosixPath(component["root"]): [WORKSPACE / consumer for consumer in component.get("consumers", [])]
            for component in tomllib.loads(text).get("components", [])}


def changed_packages(paths, packages, consumers):
    """Return the packages whose sources, or consumed external components, the paths change."""
    selected = []
    for path in map(PurePosixPath, paths):
        owners = [owning_package(path, packages)]
        for root, sources in consumers.items():
            if path.is_relative_to(root):
                owners.extend(owning_package(source, packages) for source in sources)
        for owner in owners:
            if owner and owner not in selected:
                selected.append(owner)
    return selected


def changed_paths(root, before, after):
    """List the paths a push changed; a new or rewritten branch is compared with origin/main."""
    if not before or EMPTY_REVISION.fullmatch(before) or subprocess.run(
            ["git", "-C", str(root), "cat-file", "-e", f"{before}^{{commit}}"], capture_output=True).returncode:
        before = subprocess.run(["git", "-C", str(root), "merge-base", "origin/main", after],
                                check=True, capture_output=True, text=True).stdout.strip()
    output = subprocess.run(["git", "-C", str(root), "diff", "--name-only", f"{before}..{after}"],
                            check=True, capture_output=True, text=True).stdout
    return [line for line in output.splitlines() if line]


def unique(selections):
    result = []
    for arguments in selections:
        if arguments not in result:
            result.append(arguments)
    if len(result) > MAX_SELECTIONS:
        raise SelectionError(f"At most {MAX_SELECTIONS} test selections may run in one focused job")
    return result


def main(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--package", help="Focused on-demand run: one workspace package")
    parser.add_argument("--filter", default="", help="Focused on-demand run: optional test-name filter")
    parser.add_argument("--before", help="Push: the revision before the push")
    parser.add_argument("--after", help="Push: the pushed revision")
    args = parser.parse_args(argv)
    packages = workspace_packages(args.root)
    names = set(packages.values())
    if args.package:
        selections = [selection(args.package, args.filter or None, names)]
    elif (args.root / LANE_FILE).is_file():
        selections = lane_entries((args.root / LANE_FILE).read_text(encoding="utf-8"), names)
    else:
        consumers = component_consumers((args.root / COMPONENTS).read_text(encoding="utf-8"))
        paths = changed_paths(args.root, args.before, args.after or "HEAD")
        changed = changed_packages(paths, packages, consumers)
        selections = [selection(name, None, names) for name in changed]
    for arguments in unique(selections):
        print(json.dumps(arguments))


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except SelectionError as error:
        print(f"lane tests: {error}", file=sys.stderr)
        sys.exit(2)
