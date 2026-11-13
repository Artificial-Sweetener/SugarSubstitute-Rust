# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Extraction writes exactly each pinned archive's regular members at its mount."""
import io
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from extract_source import extract
from verify_source import digest


def archive(path, members):
    with tarfile.open(path, "w:gz") as output:
        for name, data in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            output.addfile(info, io.BytesIO(data))


class ExtractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TEMP"))
        self.addCleanup(self.temporary.cleanup)
        self.bundle = Path(self.temporary.name)
        self.downloads = self.bundle / "native-build" / "downloads"
        self.downloads.mkdir(parents=True)
        archive(self.downloads / "main.tar.gz", {"main-abc/BUILD.gn": b"root", "main-abc/util/a.cc": b"a"})
        archive(self.downloads / "dep.tar.gz", {"x.h": b"dep"})
        archive(self.downloads / "tools.tar.gz", {"gn/README": b"tools"})
        self.packages = (
            ("main", "abc", digest(self.downloads / "main.tar.gz"), "", "main"),
            ("dep", "def", digest(self.downloads / "dep.tar.gz"), "third_party/dep/dep", ""),
            ("tools", "ghi", digest(self.downloads / "tools.tar.gz"), "../buildtools", ""),
        )

    def test_members_land_under_their_mounts_without_prefixes(self):
        source = extract(self.bundle, self.packages)
        self.assertEqual(source, self.bundle / "prepared" / "crashpad")
        self.assertEqual((source / "BUILD.gn").read_bytes(), b"root")
        self.assertEqual((source / "util" / "a.cc").read_bytes(), b"a")
        self.assertEqual((source / "third_party" / "dep" / "dep" / "x.h").read_bytes(), b"dep")
        self.assertEqual((self.bundle / "prepared" / "buildtools" / "gn" / "README").read_bytes(), b"tools")

    def test_mismatched_archive_rejects_before_writing_anything(self):
        archive(self.downloads / "dep.tar.gz", {"x.h": b"changed"})
        with self.assertRaisesRegex(ValueError, "identity mismatch"):
            extract(self.bundle, self.packages)
        self.assertFalse((self.bundle / "prepared").exists())

    def test_existing_prepared_sources_are_never_overwritten(self):
        (self.bundle / "prepared").mkdir()
        with self.assertRaisesRegex(ValueError, "fresh bundle"):
            extract(self.bundle, self.packages)

    def test_member_outside_the_archive_prefix_rejects(self):
        archive(self.downloads / "main.tar.gz", {"other-abc/BUILD.gn": b"root"})
        packages = (("main", "abc", digest(self.downloads / "main.tar.gz"), "", "main"),)
        with self.assertRaisesRegex(ValueError, "prefix"):
            extract(self.bundle, packages)


if __name__ == "__main__":
    unittest.main()
