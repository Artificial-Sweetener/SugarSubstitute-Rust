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
"""Prove provider download evidence stays anonymous, fresh and strictly validated."""

from __future__ import annotations

import asyncio
import json

import aiohttp
import pytest

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess
from sugarcubes.model_library.infrastructure.civitai_download_access import (
    CivitaiDownloadAccess,
    parse_download_access,
)
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
from tests.model_library.support.download_access import fixture_offer
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


def test_acquisition_rechecks_anonymous_access_after_public_presentation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Observe a new provider restriction rather than trusting an earlier public badge."""
    payload = {"availability": "Public", "requireAuth": False, "checkPermission": False}
    session = TransportSession(TransportResponse(200, json.dumps(payload).encode()))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Use the same transport and evidence owner for rendering and transfer preflight."""
        access = CivitaiDownloadAccess(CivitaiTransport(lambda: "fixture-key"))
        assert await access.version("20") == ModelDownloadAccess.PUBLIC
        payload["requireAuth"] = True
        session.response = TransportResponse(200, json.dumps(payload).encode())
        assert (
            await access.access_for(fixture_offer())
            == ModelDownloadAccess.API_KEY_REQUIRED
        )

    asyncio.run(exercise())
    assert (
        session.requests
        == [("https://civitai.com/api/v1/model-versions/mini/20", False)] * 2
    )
    assert all("Authorization" not in headers for headers in session.headers)


@pytest.mark.parametrize(
    "payload",
    [
        None,
        {},
        {"availability": "Public", "requireAuth": "false", "checkPermission": False},
    ],
)
def test_incomplete_permission_evidence_cannot_grant_public_access(
    payload: object,
) -> None:
    """Reject missing and truthy string fields before they can authorize a download."""
    with pytest.raises(ValueError, match="invalid download-access evidence"):
        parse_download_access(payload)


def test_missing_version_is_unknown_access(monkeypatch: pytest.MonkeyPatch) -> None:
    """Keep an unavailable version from being advertised as anonymous or authorized."""
    session = TransportSession(TransportResponse(404, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    assert (
        asyncio.run(CivitaiDownloadAccess(CivitaiTransport()).version("20"))
        == ModelDownloadAccess.UNKNOWN
    )
