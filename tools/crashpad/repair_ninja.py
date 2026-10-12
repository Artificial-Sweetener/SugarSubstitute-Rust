# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Repair the pinned GN cross-drive defect only in reviewed generated Ninja positions."""
import hashlib
import json
from pathlib import PureWindowsPath
import re

RELATIVE_DRIVE = re.compile(r"(?P<prefix>(?:\.\./)+)(?P<drive>[A-Za-z])(?P<escaped>\$?):(?P<suffix>/[^\s]*)")


def repair_text(text, source, output):
    source = PureWindowsPath(source)
    output = PureWindowsPath(output)
    if source.drive.upper() != "E:" or output.drive.upper() != "D:":
        raise ValueError("Reviewed source/output drive mismatch")
    root = source.as_posix()[2:].rstrip("/")
    prefix = "../" * len(output.parts)
    repaired = []
    for line in text.splitlines(keepends=True):
        matches = list(RELATIVE_DRIVE.finditer(line))
        if matches:
            path_position = line.startswith("build ") or line.startswith("include_dirs = ")
            script_position = line.startswith("  command = ") and "/write_buildflag_header.py " in line
            definition_position = line.startswith("  ldflags = ")
            if not path_position and not script_position and not definition_position:
                raise ValueError("Unexpected generated Ninja path position")
            for match in matches:
                suffix = match["suffix"]
                if match["drive"].upper() != "E" or match["prefix"] != prefix:
                    raise ValueError("Unexpected relative-drive prefix")
                if not (suffix == root or suffix.startswith(root + "/")):
                    raise ValueError("Generated path escaped pinned source")
                tail = suffix[len(root):]
                if any(part in (".", "..") for part in tail.split("/")) or any(char in suffix for char in '$:"\\'):
                    raise ValueError("Ambiguous generated source path")
                if script_position and tail != "/third_party/mini_chromium/mini_chromium/build/write_buildflag_header.py":
                    raise ValueError("Unexpected generated command source")
                if definition_position and (not line[:match.start()].endswith("/DEF:") or tail != "/handler/win/wer/crashpad_wer.def"):
                    raise ValueError("Unexpected generated link input")
            line = RELATIVE_DRIVE.sub(lambda match: "E$:" + match["suffix"], line)
        repaired.append(line)
    return "".join(repaired)


def repair_output(source, output, receipt):
    files = []
    changes = []
    # Only GN-generated Ninja files, before any compiler has run. No native
    # compiler depfile, source file or environment block is rewritten.
    for path in sorted(output.rglob("*.ninja")):
        original = path.read_bytes()
        repaired = repair_text(original.decode("utf-8"), source, output).encode("utf-8")
        if repaired != original:
            files.append((path, repaired))
            changes.append({"file": str(path.relative_to(output)),
                            "original_sha256": hashlib.sha256(original).hexdigest(),
                            "repaired_sha256": hashlib.sha256(repaired).hexdigest()})
    if not changes:
        raise ValueError("Pinned GN emitted no expected cross-drive defect")
    for path, repaired in files:
        path.write_bytes(repaired)
    receipt.write_text(json.dumps(changes, indent=2) + "\n", encoding="utf-8")
