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
"""Resolve anonymous CivitAI download permission through the shared bounded transport."""

from __future__ import annotations

import logging
import re
from urllib.parse import urlsplit

from ..domain.artifact_transport import approved_artifact_start
from ..domain.provider_releases import ModelArtifactOffer, ModelDownloadAccess
from .civitai_transport import CivitaiResourceMissing, CivitaiTransport

_LOGGER = logging.getLogger(__name__)


class CivitaiDownloadAccess:
    """Own provider access interpretation for every artifact presentation and acquisition."""

    def __init__(self, transport: CivitaiTransport) -> None:
        """Reuse the host's request budget, TLS policy and cooldown."""
        self._transport = transport

    async def version(self, version_id: str) -> ModelDownloadAccess:
        """Measure anonymous permission without leaking configured credentials into the probe."""
        if not re.fullmatch(r"[1-9][0-9]{0,15}", version_id):
            raise ValueError("Invalid CivitAI download version identity.")
        try:
            payload = await self._transport.read_json(
                f"https://civitai.com/api/v1/model-versions/mini/{version_id}",
                authenticated=False,
            )
        except CivitaiResourceMissing:
            _LOGGER.warning(
                "CivitAI download access unavailable; versionId=%s", version_id
            )
            return ModelDownloadAccess.UNKNOWN
        return parse_download_access(payload)

    async def access_for(self, offer: ModelArtifactOffer) -> ModelDownloadAccess:
        """Recheck the canonical route immediately before transfer instead of trusting cached access."""
        if not approved_artifact_start(offer.download_url, offer.provider):
            raise ValueError("The model download route is not approved.")
        if offer.provider == "openmodeldb":
            return ModelDownloadAccess.PUBLIC
        return await self.version(urlsplit(offer.download_url).path.rsplit("/", 1)[-1])


def parse_download_access(payload: object) -> ModelDownloadAccess:
    """Require explicit anonymous access evidence rather than inferring access from a URL."""
    if not isinstance(payload, dict):
        raise ValueError("CivitAI returned invalid download-access evidence.")
    availability = payload.get("availability")
    auth, permission = payload.get("requireAuth"), payload.get("checkPermission")
    if (
        not isinstance(availability, str)
        or not isinstance(auth, bool)
        or not isinstance(permission, bool)
    ):
        raise ValueError("CivitAI returned invalid download-access evidence.")
    return (
        ModelDownloadAccess.PUBLIC
        if availability.casefold() == "public" and not auth and not permission
        else ModelDownloadAccess.API_KEY_REQUIRED
    )
