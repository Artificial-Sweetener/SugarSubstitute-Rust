# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Derive the public compiled local-K1 identity from exact first-party build inputs.

The emitted receipt is reproducibility data. Runtime trust comes from the literal
public anchor and the detached signature over the actual selected images.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import stat
import struct
import subprocess

DOMAIN = b"SugarSubstitute.local-k1.source-build.v1\0"
DERIVED = "crates/sugarsubstitute-supervision/src/installed_release/local_k1_build.rs"
ANCHOR_ID = "790bf1193d5cfd7efa58436532b10c09162d5efd5200b40a9d86fc71257753a5"
SUFFIXES = {".rs", ".toml", ".lock", ".py", ".ps1", ".cs", ".cpp", ".cc", ".c", ".h", ".hpp", ".gn", ".gni", ".json", ".yml", ".yaml"}
ROOTS = {"crates", "xtask", "tools", "native", ".cargo", ".github", "governance"}


def source_input(relative: str) -> bool:
    """Select checked first-party source/configuration; exclude the derived literal."""
    path = Path(relative)
    return (relative != DERIVED and not path.is_absolute() and ".." not in path.parts
            and path.suffix in SUFFIXES
            and (len(path.parts) == 1 or path.parts[0] in ROOTS))


def regular_path(path: Path, *, directory: bool = False) -> Path:
    """Reject reparse ancestors and wrong concrete leaf types without following links."""
    absolute = path.absolute()
    for part in reversed((absolute, *absolute.parents)):
        info = part.lstat()
        if getattr(info, "st_file_attributes", 0) & 0x400 or stat.S_ISLNK(info.st_mode):
            raise ValueError("Source/output ancestry contains a reparse point")
    info = absolute.stat()
    if (directory and not stat.S_ISDIR(info.st_mode)) or (not directory and not stat.S_ISREG(info.st_mode)):
        raise ValueError("Source/output has an unexpected type")
    return absolute


def identity(anchor: bytes, revision: str, records: list[tuple[str, bytes]]) -> str:
    """Hash a canonical length-delimited inventory, independent of discovery order."""
    if len(anchor) != 411 or hashlib.sha256(anchor).hexdigest() != ANCHOR_ID:
        raise ValueError("Public anchor differs from the reviewed development identity")
    if len(revision) != 40 or any(byte not in "0123456789abcdef" for byte in revision):
        raise ValueError("Original revision is not a canonical Git SHA-1")
    ordered = sorted(records, key=lambda record: record[0])
    names = [record[0] for record in ordered]
    if not ordered or len(set(names)) != len(names) or any(not source_input(name) for name in names):
        raise ValueError("Source inventory is empty, ambiguous or contains a derived/foreign input")
    digest = hashlib.sha256(DOMAIN)
    for value in [anchor, revision.encode("ascii")]:
        digest.update(struct.pack("<Q", len(value)))
        digest.update(value)
    digest.update(struct.pack("<Q", len(ordered)))
    for name, data in ordered:
        for value in [name.encode("utf-8"), data]:
            digest.update(struct.pack("<Q", len(value)))
            digest.update(value)
    return "k1-" + digest.hexdigest()[:48]


def produce(workspace: Path, public_anchor: Path, output_parent: Path, output_name: str) -> Path:
    """Create fresh public outputs from resolved explicit roots; never access a signer."""
    root = regular_path(workspace, directory=True)
    parent = regular_path(output_parent, directory=True)
    if not output_name or Path(output_name).name != output_name or output_name in {".", ".."}:
        raise ValueError("Output name is not a single fresh directory leaf")
    anchor = regular_path(public_anchor).read_bytes()
    revision = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    listing = subprocess.run(["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"], check=True, capture_output=True).stdout
    # Cached paths include deliberate working-tree deletions; other missing inputs fail validation.
    deleted = subprocess.run(["git", "-C", str(root), "ls-files", "--deleted", "-z"], check=True, capture_output=True).stdout
    deleted_names = {name.decode("utf-8") for name in deleted.split(b"\0") if name}
    names = sorted({name.decode("utf-8") for name in listing.split(b"\0")
                    if name and name.decode("utf-8") not in deleted_names and source_input(name.decode("utf-8"))})
    records = []
    for name in names:
        path = regular_path(root / name)
        if not path.is_relative_to(root):
            raise ValueError("Source input escaped the workspace")
        records.append((name, path.read_bytes()))
    build = identity(anchor, revision, records)
    output = parent / output_name
    output.mkdir(exist_ok=False)
    literal = ("// SPDX-License-Identifier: GPL-3.0-or-later\n"
               "// Copyright (C) 2026 Artificial Sweetener and contributors\n"
               "//! Bind the signed local-K1 manifest to its compiled source inventory.\n\n"
               "/// Reproducible development build identity; regenerated before final image creation.\n"
               f'pub(super) const BUILD: &str = "{build}";\n')
    with (output / "local_k1_build.rs").open("x", encoding="utf-8", newline="\n") as stream:
        stream.write(literal)
    receipt = {"schema": 1, "build": build, "original_revision": revision,
               "public_anchor_sha256": hashlib.sha256(anchor).hexdigest(),
               "derived_output_excluded": DERIVED,
               "inputs": [{"relative": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()} for name, data in records],
               "scope": "Reproducibility facts only; no private signer access, signature or release authority."}
    with (output / "build-receipt.json").open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(receipt, stream, indent=2)
        stream.write("\n")
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--public-anchor", type=Path, required=True)
    parser.add_argument("--output-parent", type=Path, required=True)
    parser.add_argument("--output-name", required=True)
    arguments = parser.parse_args()
    output = produce(arguments.workspace, arguments.public_anchor, arguments.output_parent, arguments.output_name)
    print(f"Public compiled build identity written to {output}")


if __name__ == "__main__":
    main()
