# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Produce schema2 matching records from actual same-open-file PE/PDB pairs.

This unsigned output contains reproducibility facts. The development signing
owner retains no-write/no-delete file and ancestry pins during this operation;
runtime trust requires the compiled anchor, signature and actual pinned files.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path
from manifest import Manifest, ManifestError, Pair, encode, decode, _label
from derive_build import ANCHOR_ID, regular_path

MAX_IMAGE = 1024 * 1024 * 1024
PDB_MAGIC = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0"


class Source:
    """Read bounded ranges and stream hashes from one owned regular-file descriptor."""
    def __init__(self, path: Path):
        self.path = regular_path(path)
        self.stream = self.path.open("rb")
        import os
        self.initial = os.fstat(self.stream.fileno())
        self.length = self.initial.st_size
        if not 1 <= self.length <= MAX_IMAGE or self.initial.st_nlink != 1:
            self.stream.close()
            raise ManifestError("Selected source bound/type")

    def read(self, offset: int, count: int) -> bytes:
        if count < 0 or count > 65536 or offset < 0 or offset + count > self.length:
            raise ManifestError("Selected source range")
        self.stream.seek(offset)
        data = self.stream.read(count)
        if len(data) != count:
            raise ManifestError("Selected source short read")
        return data

    def digest(self) -> bytes:
        digest = hashlib.sha256()
        for offset in range(0, self.length, 65536):
            digest.update(self.read(offset, min(65536, self.length - offset)))
        self.validate()
        return digest.digest()

    def validate(self) -> None:
        import os
        current = os.fstat(self.stream.fileno())
        fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_nlink")
        if any(getattr(current, field) != getattr(self.initial, field) for field in fields):
            raise ManifestError("Selected source changed")

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.stream.close()


def number(data: bytes, offset: int, size: int = 4) -> int:
    if offset < 0 or offset + size > len(data):
        raise ManifestError("Image field range")
    return int.from_bytes(data[offset:offset + size], "little")


def read_pe(source: Source) -> tuple[bool, int, int, int, bytes, int]:
    """Read the actual AMD64 PE32+ image kind and sole mapped RSDS identity."""
    dos = source.read(0, 64)
    if dos[:2] != b"MZ":
        raise ManifestError("PE DOS magic")
    offset = number(dos, 60)
    header = source.read(offset, 24)
    if header[:4] != b"PE\0\0" or number(header, 4, 2) != 0x8664:
        raise ManifestError("PE machine")
    count, optional, flags = number(header, 6, 2), number(header, 20, 2), number(header, 22, 2)
    if not 1 <= count <= 96 or not 240 <= optional <= 4096 or not flags & 2:
        raise ManifestError("PE header bound")
    opt = source.read(offset + 24, 240)
    if number(opt, 0, 2) != 0x20B or not 7 <= number(opt, 108) <= 16:
        raise ManifestError("PE optional schema")
    image_size, headers = number(opt, 56), number(opt, 60)
    table = offset + 24 + optional
    if not image_size or headers > source.length or table + count * 40 > headers:
        raise ManifestError("PE header range")
    sections = []
    for index in range(count):
        section = source.read(table + index * 40, 40)
        virtual, address, raw_size, raw = [number(section, field) for field in (8, 12, 16, 20)]
        if raw + raw_size > source.length or address + max(virtual, raw_size) > image_size:
            raise ManifestError("PE section range")
        sections.append((address, raw_size, raw))

    def mapped(address: int, length: int) -> int:
        selected = [address] if address + length <= headers else []
        selected.extend(raw + address - start for start, size, raw in sections
                        if address >= start and address + length <= start + size)
        if len(selected) != 1:
            raise ManifestError("PE missing/ambiguous RVA")
        return selected[0]

    rva, size = number(opt, 160), number(opt, 164)
    if not rva or not size or size > 28 * 16 or size % 28:
        raise ManifestError("PE debug bound")
    debug = mapped(rva, size)
    identity = None
    for index in range(size // 28):
        record = source.read(debug + index * 28, 28)
        if number(record, 12) != 2:
            continue
        if identity is not None:
            raise ManifestError("PE ambiguous CodeView")
        length, address, raw = [number(record, field) for field in (16, 20, 24)]
        if not 25 <= length <= 4096 or mapped(address, length) != raw:
            raise ManifestError("PE CodeView range")
        rsds = source.read(raw, length)
        if rsds[:4] != b"RSDS" or rsds[24] == 0 or rsds[-1] != 0 or 0 in rsds[24:-1]:
            raise ManifestError("PE CodeView schema/name")
        guid, age = rsds[4:20], number(rsds, 20)
        if guid == bytes(16) or not age:
            raise ManifestError("PE CodeView identity")
        identity = (guid, age)
    if identity is None:
        raise ManifestError("PE missing CodeView")
    return bool(flags & 0x2000), image_size, number(header, 8), number(opt, 64), *identity


def read_pdb(source: Source) -> tuple[bytes, int]:
    """Resolve only the actual MSF7 information stream through bounded block maps."""
    header = source.read(0, 56)
    if header[:32] != PDB_MAGIC:
        raise ManifestError("PDB MSF magic")
    block, free, count, directory, page_map = [number(header, field) for field in (32, 36, 40, 44, 52)]
    if (block not in (512, 1024, 2048, 4096) or free not in (1, 2) or count < 4
            or block * count != source.length or not 16 <= directory <= block * (block // 4)):
        raise ManifestError("PDB MSF bound")

    def valid(page: int):
        if page >= count or page == 0 or page % block in (1, 2):
            raise ManifestError("PDB block range")

    valid(page_map)
    pages = (directory + block - 1) // block
    table = source.read(page_map * block, pages * 4)
    directory_pages = [number(table, index * 4) for index in range(pages)]
    for page in directory_pages:
        valid(page)
    if page_map in directory_pages or len(set(directory_pages)) != pages:
        raise ManifestError("PDB directory alias")

    def field(offset: int) -> int:
        if offset + 4 > directory or offset % 4:
            raise ManifestError("PDB directory range")
        page = directory_pages[offset // block]
        return number(source.read(page * block + offset % block, 4), 0)

    streams = field(0)
    if not 2 <= streams <= 65536 or 4 + streams * 4 > directory:
        raise ManifestError("PDB stream bound")
    first, info = field(4), field(8)
    first_pages = 0 if first == 0xFFFFFFFF else (first + block - 1) // block
    info_pages = (info + block - 1) // block
    start = 4 + streams * 4 + first_pages * 4
    if info < 28 or info == 0xFFFFFFFF or info_pages > count or start + info_pages * 4 > directory:
        raise ManifestError("PDB information bound")
    page = field(start)
    valid(page)
    if page == page_map or page in directory_pages:
        raise ManifestError("PDB information alias")
    data = source.read(page * block, 28)
    if number(data, 0) not in (20000404, 20030901, 20091201, 20140508):
        raise ManifestError("PDB information version")
    age, guid = number(data, 8), data[12:28]
    if not age or guid == bytes(16):
        raise ManifestError("PDB information identity")
    return guid, age


def pair(root: Path, image: str, pdb: str, roles: int, *, library: bool) -> Pair:
    """Hash and parse each original descriptor, rejecting a mismatched PE/PDB pair."""
    _label(image, {"dll" if library else "exe"})
    _label(pdb, {"pdb"})
    with Source(root / image) as pe, Source(root / pdb) as debug:
        is_library, size, timestamp, checksum, guid, age = read_pe(pe)
        if is_library != library or read_pdb(debug) != (guid, age):
            raise ManifestError("Actual PE kind/PDB GUID-age mismatch")
        return Pair(roles, image, pdb, pe.length, pe.digest(), debug.length, debug.digest(),
                    size, timestamp, checksum, guid, age)


def build_identity(path: Path) -> str:
    """Read the exact source literal that the final host build consumes."""
    with Source(path) as source:
        if source.length > 65536:
            raise ManifestError("Compiled literal source bound")
        text = source.read(0, source.length).decode("utf-8")
    matches = re.findall(r'pub\(super\) const BUILD: &str = "(k1-[0-9a-f]{48})";', text)
    if len(matches) != 1:
        raise ManifestError("Compiled literal source identity")
    return matches[0]


def produce(root: Path, host: str, literal: Path, anchor: Path) -> bytes:
    """Produce exactly host/helper3, capture-DLL8 and handler16 schema2 records."""
    root = regular_path(root, directory=True)
    _label(host, {"exe"})
    with Source(anchor) as source:
        blob = source.read(0, source.length)
    if len(blob) != 411 or hashlib.sha256(blob).hexdigest() != ANCHOR_ID:
        raise ManifestError("Reviewed development anchor mismatch")
    pairs = (pair(root, host, host[:-4] + ".pdb", 3, library=False),
             pair(root, "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb", 8, library=True),
             pair(root, "crashpad_handler.exe", "crashpad_handler.pdb", 16, library=False))
    value = Manifest(bytes.fromhex(ANCHOR_ID), build_identity(literal), tuple(sorted(pairs, key=lambda pair: pair.pe_label)), 2)
    encoded = encode(value)
    if decode(encoded) != value:
        raise ManifestError("Produced manifest roundtrip")
    return encoded


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-root", type=Path, required=True)
    parser.add_argument("--host-image", required=True)
    parser.add_argument("--build-literal", type=Path, required=True)
    parser.add_argument("--public-anchor", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    arguments = parser.parse_args()
    data = produce(arguments.release_root, arguments.host_image, arguments.build_literal, arguments.public_anchor)
    root = regular_path(arguments.output_root, directory=True)
    with (root / "sugarsubstitute-release-v1.manifest").open("xb") as stream:
        stream.write(data)
        stream.flush()
        import os
        os.fsync(stream.fileno())
    value = decode(data)
    receipt = {"schema": 1, "scope": "Unsigned actual-file matching facts; not release authority",
               "build": value.build, "key_id": value.key_id.hex(), "manifest_sha256": hashlib.sha256(data).hexdigest(),
               "images": [{"image": p.pe_label, "pdb": p.pdb_label, "roles": p.roles,
                           "image_sha256": p.pe_sha256.hex(), "pdb_sha256": p.pdb_sha256.hex(),
                           "pdb_guid": p.pdb_guid.hex(), "pdb_age": p.pdb_age} for p in value.pairs]}
    with (root / "local-k1-manifest-inputs.json").open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(receipt, stream, indent=2)
        stream.write("\n")
    print("Produced actual schema2 development manifest from three matched PE/PDB pairs")


if __name__ == "__main__":
    main()
