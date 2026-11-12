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
"""Prove reference byte-size normalization and exact transfer evidence before publication."""

from __future__ import annotations


from tests.model_library.support.download_access import fixture_download_authorization
import asyncio
from dataclasses import replace
import hashlib
from pathlib import Path

import aiohttp
import pytest

from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
)
from sugarcubes.model_library.infrastructure.civitai_artifact_codec import (
    parse_civitai_artifact,
)
from tests.model_library.support.civitai_discovery import discovery_file
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer


def fixture_offer() -> ModelArtifactOffer:
    """Use exact seven-byte acquisition evidence without loading any model weights."""
    return ModelArtifactOffer(
        "30",
        "fixture.safetensors",
        hashlib.sha256(b"fixture").hexdigest(),
        7,
        "https://civitai.com/api/download/models/20",
        "loras",
    )


@pytest.mark.parametrize("declared", [6, 8])
def test_matching_hash_does_not_override_exact_declared_size(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, declared: int
) -> None:
    """Reject both excessive and short bytes even when their SHA256 matches the reviewed offer."""
    session = TransportSession(
        TransportResponse(200, b"fixture"), expected_timeout=7200
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    downloads = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path), lambda: None, fixture_download_authorization()
    )
    progress: list[tuple[int, int]] = []
    with pytest.raises(ValueError, match="declared size"):
        asyncio.run(
            downloads.install(
                replace(fixture_offer(), size_bytes=declared),
                lambda received, total: progress.append((received, total)),
            )
        )
    assert not tuple(tmp_path.iterdir())
    assert all(received <= total for received, total in progress)


@pytest.mark.parametrize("declared", ["6", "8", "-1", "unknown", "9" * 21])
def test_response_length_disagreement_stops_before_streaming(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, declared: str
) -> None:
    """Reject stale HTTP size evidence before receiving otherwise hash-matching fixture bytes."""
    response = TransportResponse(200, b"fixture")
    response.headers["Content-Length"] = declared
    session = TransportSession(response, expected_timeout=7200)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    progress: list[tuple[int, int]] = []
    with pytest.raises(ValueError, match="provider metadata"):
        asyncio.run(
            ModelArtifactDownloads(
                FixtureFileRoots(tmp_path),
                lambda: None,
                fixture_download_authorization(),
            ).install(
                fixture_offer(),
                lambda received, total: progress.append((received, total)),
            )
        )
    assert not tuple(tmp_path.iterdir()) and not progress


@pytest.mark.parametrize(
    "kilobytes,expected", [(0.0063, 6), (0.0069, 7), (0.0001, None)]
)
def test_civitai_size_uses_reference_rounding(
    kilobytes: float, expected: int | None
) -> None:
    """Keep all acquisition callers on Substitute's rounded integer-byte evidence."""
    payload = discovery_file(hashlib.sha256(b"fixture").hexdigest())
    payload["sizeKB"] = kilobytes
    offer = parse_civitai_artifact(payload, "checkpoints")
    assert (offer.size_bytes if offer is not None else None) == expected
