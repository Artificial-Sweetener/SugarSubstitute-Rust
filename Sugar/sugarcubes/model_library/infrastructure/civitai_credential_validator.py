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
"""Validate explicit CivitAI keys through the shared bounded transport and cooldown."""

from __future__ import annotations

import logging

import aiohttp

from ..application.credential_service import CredentialTestResult
from .civitai_transport import CivitaiResourceMissing, CivitaiTransport
from .civitai_codec import parse_civitai_version
from .provider_credentials import model_request_credentials

_LOGGER = logging.getLogger(__name__)


class CivitaiApiKeyValidator:
    """Use Substitute's lightweight zero-hash lookup without bypassing shared request limits."""

    def __init__(self, transport: CivitaiTransport) -> None:
        """Reuse the metadata/discovery transport rather than constructing a credential-only client."""
        self._transport = transport

    async def test(self, api_key: str) -> CredentialTestResult:
        """Treat authenticated found/not-found as success and redact other provider failure details."""
        try:
            with model_request_credentials(api_key):
                payload = await self._transport.read_json(
                    "https://civitai.com/api/v1/model-versions/by-hash/" + "0" * 64
                )
                parse_civitai_version(payload)
        except CivitaiResourceMissing:
            return CredentialTestResult(True, "CivitAI API key works.")
        except (aiohttp.ClientError, OSError, TimeoutError, ValueError) as error:
            _LOGGER.warning(
                "CivitAI credential test failed; errorType=%s", type(error).__name__
            )
            return CredentialTestResult(False, "CivitAI API key test failed.")
        return CredentialTestResult(True, "CivitAI API key works.")
