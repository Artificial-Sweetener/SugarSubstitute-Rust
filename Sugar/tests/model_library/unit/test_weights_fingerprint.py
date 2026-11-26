#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Prove a safetensors file is identified by its weights whatever its header says."""

from __future__ import annotations

import asyncio
import hashlib
import json
from pathlib import Path

from sugarcubes.model_library.domain.models import ModelEvidence
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from tests.model_library.support.model_roots import FixtureFileRoots

_PAYLOAD = bytes(range(256)) * 8


def _safetensors(metadata: dict[str, str]) -> bytes:
    """Lay out a file as safetensors does: header length, JSON header, then the weights."""
    header = json.dumps({"__metadata__": metadata}).encode("utf-8")
    return len(header).to_bytes(8, "little") + header + _PAYLOAD


def _evidence(root: Path) -> dict[str, ModelEvidence]:
    """Read the fixture files through the approved root, keyed by file name."""
    return {item.relative_path: item for item in FixtureFileRoots(root).list_files()}


def test_files_with_the_same_weights_and_different_headers_share_a_weights_hash(
    tmp_path: Path,
) -> None:
    """Hash only the bytes after the header, so a rewritten header changes nothing."""
    short, long = _safetensors({}), _safetensors({"note": "rewritten " * 40})
    assert len(short) != len(long)
    (tmp_path / "short.safetensors").write_bytes(short)
    (tmp_path / "long.SafeTensors").write_bytes(long)
    evidence = _evidence(tmp_path)
    fingerprints = ApprovedModelFingerprints(FixtureFileRoots(tmp_path))

    async def exercise() -> tuple[str | None, str | None, str, str]:
        """Read both hashes of both files."""
        return (
            await fingerprints.weights_fingerprint(evidence["short.safetensors"]),
            await fingerprints.weights_fingerprint(evidence["long.SafeTensors"]),
            await fingerprints.fingerprint(evidence["short.safetensors"]),
            await fingerprints.fingerprint(evidence["long.SafeTensors"]),
        )

    short_weights, long_weights, short_full, long_full = asyncio.run(exercise())
    assert short_weights == long_weights == hashlib.sha256(_PAYLOAD).hexdigest()
    assert short_full == hashlib.sha256(short).hexdigest()
    assert long_full == hashlib.sha256(long).hexdigest()
    assert len({short_weights, short_full, long_full}) == 3


def test_a_file_that_is_not_safetensors_has_no_weights_hash(tmp_path: Path) -> None:
    """Answer none by name alone, even for bytes laid out like safetensors."""
    (tmp_path / "fixture.pth").write_bytes(_safetensors({}))
    evidence = _evidence(tmp_path)["fixture.pth"]
    fingerprints = ApprovedModelFingerprints(FixtureFileRoots(tmp_path))
    assert asyncio.run(fingerprints.weights_fingerprint(evidence)) is None
    assert asyncio.run(fingerprints.fingerprint(evidence)) == (
        hashlib.sha256(_safetensors({})).hexdigest()
    )


def test_a_safetensors_name_without_a_readable_header_has_no_weights_hash(
    tmp_path: Path,
) -> None:
    """Answer none when the stated header leaves no weights, instead of hashing nothing."""
    (tmp_path / "empty-header.safetensors").write_bytes(bytes(8) + _PAYLOAD)
    (tmp_path / "all-header.safetensors").write_bytes(
        (4096).to_bytes(8, "little") + b"{}"
    )
    fingerprints = ApprovedModelFingerprints(FixtureFileRoots(tmp_path))
    for evidence in _evidence(tmp_path).values():
        assert asyncio.run(fingerprints.weights_fingerprint(evidence)) is None
