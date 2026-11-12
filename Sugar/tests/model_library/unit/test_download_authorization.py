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
"""Prove acquisition access is enforced at the shared transfer boundary, independently of UI entry points."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path

import aiohttp
import pytest

from sugarcubes.model_library.application.download_authorization import (
    ModelDownloadAuthorization,
)
from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess
from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
)
from tests.model_library.support.download_access import FixtureDownloadAccess
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)
from tests.model_library.support.download_access import fixture_offer


@pytest.mark.parametrize("method", ["install", "install_to"])
@pytest.mark.parametrize(
    "access", [ModelDownloadAccess.API_KEY_REQUIRED, ModelDownloadAccess.UNKNOWN]
)
def test_missing_access_blocks_all_transfer_paths_before_io(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    method: str,
    access: ModelDownloadAccess,
) -> None:
    """Reject stale public offers through the same owner before any temporary file or remote transfer."""

    def forbid_network(**kwargs: object) -> None:
        """Fail if denied acquisition tries to open a model transfer session."""
        raise AssertionError("Denied acquisition opened the transfer.")

    monkeypatch.setattr(aiohttp, "ClientSession", forbid_network)
    evidence = FixtureDownloadAccess(access)
    installer = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path),
        lambda: None,
        ModelDownloadAuthorization(evidence, lambda: None),
    )
    offer = replace(fixture_offer(), access=ModelDownloadAccess.PUBLIC)
    with pytest.raises(ValueError, match="API key|could not be verified"):
        asyncio.run(
            installer.install(offer)
            if method == "install"
            else installer.install_to(offer, "family/fixture.safetensors")
        )
    assert evidence.requests == [offer]
    assert not tuple(tmp_path.iterdir())


@pytest.mark.parametrize(
    "access,key",
    [
        (ModelDownloadAccess.PUBLIC, None),
        (ModelDownloadAccess.API_KEY_REQUIRED, "fixture-key"),
    ],
)
def test_current_access_allows_verified_public_or_authenticated_transfer(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    access: ModelDownloadAccess,
    key: str | None,
) -> None:
    """Allow both valid routes through actual transfer, hash validation and publication."""
    session = TransportSession(
        TransportResponse(200, b"fixture"), expected_timeout=7200
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    evidence = FixtureDownloadAccess(access)

    def source() -> str | None:
        """Resolve the fixture credential without sharing it with other transfers."""
        return key

    installer = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path), source, ModelDownloadAuthorization(evidence, source)
    )
    assert asyncio.run(installer.install(fixture_offer())) == "fixture.safetensors"
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
    assert evidence.requests == [fixture_offer()]
    assert not tuple(tmp_path.glob("*.download"))


def test_local_exact_artifact_does_not_require_remote_access(tmp_path: Path) -> None:
    """Keep an installed verified model usable even without a key or provider availability."""
    (tmp_path / "renamed.safetensors").write_bytes(b"fixture")
    evidence = FixtureDownloadAccess(ModelDownloadAccess.UNKNOWN)
    installer = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path),
        lambda: None,
        ModelDownloadAuthorization(evidence, lambda: None),
    )
    assert asyncio.run(installer.install(fixture_offer())) == "renamed.safetensors"
    assert not evidence.requests
