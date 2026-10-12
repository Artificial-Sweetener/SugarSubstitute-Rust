# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify immutable complete source admission with isolated archive boundaries."""
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from verify_source import AUTHORED, archive_files, authored_inputs, verify


class SourceTests(unittest.TestCase):
    def setUp(self):
        # All generated fixtures honor the explicitly supplied D: TEMP/TMP.
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "cache/crashpad"
        self.source.mkdir(parents=True)
        (self.source.parent / "buildtools").mkdir()
        self.authored = self.root / "authored"
        self.authored.mkdir()
        for name in AUTHORED:
            (self.authored / name).write_bytes(name.encode())
        (self.source / "sugarsubstitute").mkdir()
        for name in AUTHORED:
            (self.source / "sugarsubstitute" / name).write_bytes(name.encode())
        self.downloads = self.root / "downloads"
        self.downloads.mkdir()
        self.archive = self.downloads / "fixture.tar.gz"
        self.write_archive("fixture-pin/input.cc", b"immutable")
        (self.source / "input.cc").write_bytes(b"immutable")
        self.packages = (("fixture", "pin", hashlib.sha256(self.archive.read_bytes()).hexdigest(), "", "fixture"),)
        self.receipt = self.root / "receipt.json"

    def write_archive(self, name, data, kind=tarfile.REGTYPE):
        with tarfile.open(self.archive, "w:gz") as archive:
            entry = tarfile.TarInfo(name)
            entry.type = kind
            entry.size = len(data) if kind == tarfile.REGTYPE else 0
            archive.addfile(entry, io.BytesIO(data) if entry.size else None)

    def check(self):
        return verify(self.source, self.authored, self.downloads, self.receipt, self.packages)

    def test_complete_inventory_hashes_actual_immutable_and_authored_files(self):
        result = self.check()
        self.assertEqual(len(result["files"]), 7)
        self.assertEqual(json.loads(self.receipt.read_text()), result)
        self.assertEqual(result, self.check())

    def test_unknown_source_is_rejected_before_receipt(self):
        (self.source / "extra.cc").write_text("unknown")
        with self.assertRaisesRegex(ValueError, "Unknown extracted"):
            self.check()
        self.assertFalse(self.receipt.exists())

    def test_modified_and_missing_source_are_rejected(self):
        path = self.source / "input.cc"
        path.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "differs"):
            self.check()
        path.unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            self.check()

    def test_modified_archive_is_rejected(self):
        self.archive.write_bytes(b"not reviewed")
        with self.assertRaisesRegex(ValueError, "archive identity"):
            self.check()

    def test_authored_unknown_or_directory_is_rejected(self):
        extra = self.authored / "extra.cc"
        extra.write_text("unknown")
        with self.assertRaisesRegex(ValueError, "six files"):
            authored_inputs(self.authored)
        extra.unlink()
        path = self.authored / "BUILD.gn"
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "regular file"):
            authored_inputs(self.authored)

    def test_traversal_foreign_prefix_link_and_duplicate_archive_members_are_rejected(self):
        for name, kind in (("fixture-pin/../outside", tarfile.REGTYPE), ("foreign-pin/input", tarfile.REGTYPE), ("fixture-pin/link", tarfile.SYMTYPE)):
            self.write_archive(name, b"", kind)
            with self.assertRaises(ValueError):
                list(archive_files(self.archive, "fixture-pin/"))
        with tarfile.open(self.archive, "w:gz") as archive:
            for _ in range(2):
                entry = tarfile.TarInfo("fixture-pin/input")
                archive.addfile(entry)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            list(archive_files(self.archive, "fixture-pin/"))

    def test_authored_copy_changed_is_rejected(self):
        (self.source / "sugarsubstitute/capture_bridge.cc").write_bytes(b"modified")
        with self.assertRaisesRegex(ValueError, "differs"):
            self.check()


if __name__ == "__main__":
    unittest.main()
