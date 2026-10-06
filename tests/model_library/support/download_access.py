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
"""Replace only external provider access while preserving production acquisition policy."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
import hashlib
import json
import aiohttp
from tests.model_library.support.provider_transport import (
    TransportSession,
    TransportResponse,
)

from sugarcubes.model_library.application.download_authorization import (
    ModelDownloadAuthorization,
)
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelDownloadAccess,
)


@dataclass
class FixtureDownloadAccess:
    """Retain deterministic provider evidence and observe every preflight request."""

    access: ModelDownloadAccess = ModelDownloadAccess.PUBLIC
    requests: list[ModelArtifactOffer] = field(default_factory=list)

    async def access_for(self, offer: ModelArtifactOffer) -> ModelDownloadAccess:
        """Return exact fixture evidence without replacing installer decisions."""
        self.requests.append(offer)
        return self.access


def fixture_download_authorization(
    credentials: Callable[[], str | None] = lambda: None,
) -> ModelDownloadAuthorization:
    """Supply anonymous public access for transport-focused tiny artifact proofs."""
    return ModelDownloadAuthorization(FixtureDownloadAccess(), credentials)


def fixture_offer() -> ModelArtifactOffer:
    """Use tiny bytes with an actual verified SHA256 rather than any model weights."""
    return ModelArtifactOffer(
        "30",
        "fixture.safetensors",
        hashlib.sha256(b"fixture").hexdigest(),
        7,
        "https://civitai.com/api/download/models/20",
        "loras",
    )


class AccessTransportSession(TransportSession):
    """Route only tiny external access probes separately from fixture artifact bytes."""

    def __init__(self, content: bytes = b"fixture") -> None:
        """Retain the exact transfer content while exposing explicit anonymous access."""
        super().__init__(TransportResponse(200, content), expected_timeout=7200)
        self._artifact = content

    def factory(
        self,
        *,
        timeout: aiohttp.ClientTimeout,
        headers: dict[str, str] | None = None,
        connector: aiohttp.BaseConnector | None = None,
    ) -> TransportSession:
        """Admit the production metadata and artifact timeout contracts at the external seam."""
        assert timeout.total in (20, 7200)
        self._expected_timeout = int(timeout.total)
        return super().factory(timeout=timeout, headers=headers, connector=connector)

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Serve a mini-version JSON probe and verified fixture bytes without mocking policy."""
        content = (
            json.dumps(
                {
                    "availability": "Public",
                    "requireAuth": False,
                    "checkPermission": False,
                }
            ).encode()
            if "/model-versions/mini/" in url
            else self._artifact
        )
        self.response = TransportResponse(200, content)
        return super().get(url, allow_redirects=allow_redirects, headers=headers)
