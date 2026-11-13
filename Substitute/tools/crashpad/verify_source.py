# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify every extracted input against immutable archives and reviewed derivatives."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import tarfile
from prepare_source import PINNED, transform

AUTHORED = frozenset({"BUILD.gn", "capture_bridge.h", "capture_bridge.cc", "owned_start.h",
                     "owned_registration.cc", "owned_registration_tests.cc", "owned_connection.h", "owned_connection.cc",
                     "capture_endpoint.h", "capture_source.h", "capture_source.cc", "capture_source_tests.cc",
                     "capture_attach_v2.h", "capture_attach_validation.h", "capture_attach_validation.cc", "capture_attach_validation_tests.cc",
                     "owned_attach.h", "owned_attach.cc", "owned_attach_protocol.cc", "owned_attach_tests.cc",
                     "capture_event.h", "capture_event.cc", "capture_event_tests.cc",
                     "capture_attach_boundary.h", "capture_attach_boundary.cc", "capture_attach_boundary_tests.cc",
                     "owned_attach_test_support.h", "capture_runtime_tests.cc", "capture_runtime_test_source.py",
                     "bounded_metadata.h", "bounded_metadata.cc", "bounded_metadata_tests.cc", "incident_bridge.h", "incident_bridge.cc", "incident_bridge_tests.cc",
                     "finished_report_abi.h", "finished_report.h", "finished_report.cc", "finished_io.h", "finished_io.cc",
                     "finished_digest.h", "finished_digest.cc", "finished_query.cc", "finished_disposition.cc", "finished_retirement.cc",
                     "finished_report_tests.cc", "finished_windows_tests.cc"})
PACKAGES = (
    ("crashpad", "60dd943f48d77dc8d05dabc04badbd8561d0b8c4", "96dfd1ecdf44095f1feaf893ce189d267707d11b5866f8f3474ee34188bdc2b8", "", "crashpad"),
    ("mini_chromium", "e5169551c51f3a52eee36b3b03f219cefe380237", "e35ed2d041232fde0bbfbc1517431c3902890526c696d64ee062dc370baf2b9d", "third_party/mini_chromium/mini_chromium", "mini_chromium"),
    ("zlib", "fef58692c1d7bec94c4ed3d030a45a1832a9615d", "d972ae20ced03876b304a8d27e3513bca49a3ac944c2ad65aae76e45e733476d", "third_party/zlib/zlib", ""),
    ("googletest", "3983f67e32fb3e9294487b9d4f9586efa6e5d088", "900f7ebff191f60d963822341f3016ef7a58a108550230f73518c8491813fccc", "third_party/googletest/googletest", "googletest"),
    ("buildtools", "efa920ce144e4dc1c1841e73179cd7e23b9f0d5e", "cdad8cd6aa20da36f9b4f061cf0096fe0c142adcb39e7a0c31f3379381579173", "../buildtools", ""),
)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def regular(path):
    status = path.lstat()
    if path.is_symlink() or getattr(status, "st_file_attributes", 0) & 0x400:
        raise ValueError("Reparse/link source input is forbidden")


def authored_inputs(authored):
    regular(authored)
    paths = list(authored.iterdir())
    if {path.name for path in paths} != AUTHORED:
        raise ValueError("Authored native source inventory differs from reviewed exact source set")
    for path in paths:
        regular(path)
        if not path.is_file():
            raise ValueError("Authored native source must be a regular file")
    return {path.name: digest(path) for path in paths}


def archive_files(archive_path, prefix):
    """Stream regular source files; reject names that could escape extraction."""
    seen = set()
    with tarfile.open(archive_path, mode="r|gz") as archive:
        for member in archive:
            name = member.name
            if member.isdir() and name == prefix.rstrip("/"):
                continue
            if not name.startswith(prefix) or "\\" in name or ":" in name:
                raise ValueError("Unexpected immutable archive prefix")
            relative = name[len(prefix):]
            parts = PurePosixPath(relative).parts
            if relative.startswith("/") or any(part in (".", "..") for part in parts):
                raise ValueError("Unsafe immutable archive member")
            if member.isdir():
                continue
            if not member.isfile() or not relative or relative in seen:
                raise ValueError("Nonregular or duplicate immutable archive member")
            seen.add(relative)
            stream = archive.extractfile(member)
            if stream is None:
                raise ValueError("Immutable archive member is unavailable")
            with stream:
                # Only the reviewed patch inputs need retained bytes.
                yield relative, stream


def verify(source, authored, downloads, receipt, packages=PACKAGES):
    author_hashes = authored_inputs(authored)
    expected = {}
    roots = [source, source.parent / "buildtools"]
    archive_receipts = []
    for package, revision, archive_hash, mount, prefix_name in packages:
        archive = downloads / (package + ".tar.gz")
        regular(archive)
        if digest(archive) != archive_hash:
            raise ValueError("Immutable archive identity mismatch")
        root = source / mount
        archive_receipts.append({"package": package, "revision": revision, "sha256": archive_hash})
        prefix = prefix_name + "-" + revision + "/" if prefix_name else ""
        for name, stream in archive_files(archive, prefix):
            key = (root / name).resolve()
            patch_name = (mount + "/" + name).lstrip("/") if mount else name
            if patch_name in PINNED:
                raw = stream.read(2 * 1024 * 1024 + 1)
                if len(raw) > 2 * 1024 * 1024 or hashlib.sha256(raw).hexdigest() != PINNED[patch_name]:
                    raise ValueError("Reviewed derivative input mismatch")
                value = hashlib.sha256(transform(patch_name, raw.decode("utf-8")).encode("utf-8")).hexdigest()
            else:
                value = hashlib.file_digest(stream, "sha256").hexdigest()
            if key in expected:
                raise ValueError("Overlapping immutable source packages")
            expected[key] = value
    for name, value in author_hashes.items():
        expected[(source / "sugarsubstitute" / name).resolve()] = value
    observed = set()
    inventory = []
    for root in roots:
        regular(root)
        for directory, folders, files in os.walk(root, followlinks=False):
            for name in folders:
                regular(Path(directory) / name)
            for name in files:
                path = Path(directory) / name
                regular(path)
                if not path.is_file():
                    raise ValueError("Nonregular extracted source")
                key = path.resolve()
                if key in observed or key not in expected:
                    raise ValueError("Unknown extracted source file")
                observed.add(key)
                value = digest(path)
                if value != expected[key]:
                    raise ValueError("Extracted source differs from immutable/reviewed input")
                inventory.append({"root": root.name, "file": path.relative_to(root).as_posix(), "sha256": value})
    if observed != expected.keys():
        raise ValueError("Immutable source files are missing")
    result = {"archives": archive_receipts, "files": sorted(inventory, key=lambda item: (item["root"], item["file"]))}
    receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result
