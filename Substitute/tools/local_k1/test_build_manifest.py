# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify actual same-open-file producer behavior with isolated deterministic images."""
import hashlib
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
from build_manifest import Source, read_pe, read_pdb, produce, pair, build_identity, PDB_MAGIC
from manifest import decode, ManifestError

GUID = bytes(range(1, 17))


def pe(*, library=False, guid=GUID, age=1):
    image = bytearray(4096)
    image[:2] = b"MZ"
    struct.pack_into("<I", image, 60, 128)
    image[128:132] = b"PE\0\0"
    struct.pack_into("<HHI", image, 132, 0x8664, 1, 77)
    struct.pack_into("<HH", image, 148, 240, 0x2022 if library else 0x22)
    opt = 152
    struct.pack_into("<H", image, opt, 0x20B)
    for offset, value in ((56, 0x2000), (60, 512), (64, 99), (108, 16), (160, 0x1100), (164, 28)):
        struct.pack_into("<I", image, opt + offset, value)
    struct.pack_into("<IIII", image, 392 + 8, 2048, 0x1000, 2048, 512)
    data = b"RSDS" + guid + struct.pack("<I", age) + b"owner.pdb\0"
    for offset, value in ((12, 2), (16, len(data)), (20, 0x1140), (24, 832)):
        struct.pack_into("<I", image, 768 + offset, value)
    image[832:832 + len(data)] = data
    return bytes(image)


def pdb(*, guid=GUID, age=1):
    image = bytearray(6 * 512)
    image[:32] = PDB_MAGIC
    struct.pack_into("<IIIIII", image, 32, 512, 1, 6, 16, 0, 3)
    struct.pack_into("<I", image, 3 * 512, 4)
    struct.pack_into("<IIII", image, 4 * 512, 2, 0xFFFFFFFF, 28, 5)
    struct.pack_into("<III", image, 5 * 512, 20000404, 0, age)
    image[5 * 512 + 12:5 * 512 + 28] = guid
    return bytes(image)


class ProducerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TEMP"))
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.image = self.root / "host.exe"
        self.debug = self.root / "host.pdb"
        self.image.write_bytes(pe())
        self.debug.write_bytes(pdb())

    def test_same_descriptor_pair_hashes_match_complete_actual_bytes(self):
        actual = pair(self.root, "host.exe", "host.pdb", 3, library=False)
        self.assertEqual(actual.pe_sha256, hashlib.sha256(pe()).digest())
        self.assertEqual(actual.pdb_sha256, hashlib.sha256(pdb()).digest())
        self.assertEqual((actual.pdb_guid, actual.pdb_age), (GUID, 1))

    def test_wrong_actual_pdb_guid_and_age_reject(self):
        for data in (pdb(guid=bytes([9]) * 16), pdb(age=2)):
            self.debug.write_bytes(data)
            with self.assertRaises(ManifestError):
                pair(self.root, "host.exe", "host.pdb", 3, library=False)

    def test_library_kind_cannot_substitute_executable_role(self):
        self.image.write_bytes(pe(library=True))
        with self.assertRaises(ManifestError):
            pair(self.root, "host.exe", "host.pdb", 3, library=False)

    def test_foreign_machine_and_ambiguous_rva_reject(self):
        for field, value in ((132, 0x14C), (392 + 12, 0)):
            image = bytearray(pe())
            struct.pack_into("<H" if field == 132 else "<I", image, field, value)
            self.image.write_bytes(image)
            with Source(self.image) as source, self.assertRaises(ManifestError):
                read_pe(source)

    def test_pdb_directory_and_information_aliases_reject(self):
        for offset, page in ((3 * 512, 3), (4 * 512 + 12, 4)):
            image = bytearray(pdb())
            struct.pack_into("<I", image, offset, page)
            self.debug.write_bytes(image)
            with Source(self.debug) as source, self.assertRaises(ManifestError):
                read_pdb(source)

    def test_read_range_and_short_source_are_bounded(self):
        with Source(self.image) as source:
            for offset, size in ((-1, 1), (0, 65537), (4096, 1)):
                with self.assertRaises(ManifestError):
                    source.read(offset, size)

    def test_source_change_is_observed_before_matching_record(self):
        with Source(self.image) as source:
            with self.image.open("r+b") as writer:
                writer.seek(4000)
                writer.write(b"x")
                writer.flush()
                os.fsync(writer.fileno())
            with self.assertRaises(ManifestError):
                source.validate()

    def test_compiled_source_literal_requires_exact_one_supported_identity(self):
        path = self.root / "local_k1_build.rs"
        path.write_text('pub(super) const BUILD: &str = "k1-' + "a" * 48 + '";\n')
        self.assertEqual(build_identity(path), "k1-" + "a" * 48)
        path.write_text(path.read_text() * 2)
        with self.assertRaises(ManifestError):
            build_identity(path)

    def test_foreign_anchor_rejects_before_image_selection(self):
        anchor = self.root / "anchor.blob"
        anchor.write_bytes(bytes(411))
        with self.assertRaises(ManifestError):
            produce(self.root, "host.exe", self.root / "absent.rs", anchor)

    def test_actual_three_pairs_produce_schema2_roles_and_sorted_labels(self):
        # This unsigned fixture binds no runtime trust. Its synthetic public bytes
        # allow the complete file-producing behavior to be tested without a key.
        blob=bytes(range(256))+bytes(range(155))
        anchor=self.root / "anchor.blob"
        anchor.write_bytes(blob)
        literal=self.root / "local_k1_build.rs"
        literal.write_text('pub(super) const BUILD: &str = "k1-' + "b" * 48 + '";\n')
        (self.root / "sugarsubstitute_crashpad_client.dll").write_bytes(pe(library=True))
        (self.root / "sugarsubstitute_crashpad_client.pdb").write_bytes(pdb())
        (self.root / "crashpad_handler.exe").write_bytes(pe())
        (self.root / "crashpad_handler.pdb").write_bytes(pdb())
        with patch("build_manifest.ANCHOR_ID", hashlib.sha256(blob).hexdigest()):
            value=decode(produce(self.root,"host.exe",literal,anchor))
        self.assertEqual(value.schema,2)
        self.assertEqual([item.roles for item in value.pairs],[16,3,8])
        self.assertEqual([item.pe_label for item in value.pairs],sorted(item.pe_label for item in value.pairs))


if __name__ == "__main__":
    unittest.main()
