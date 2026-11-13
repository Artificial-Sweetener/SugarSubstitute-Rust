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
"""Share bounded CivitAI transport, credentials, concurrency, and cooldown across consumers."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
import json
import re
import time
from urllib.parse import urljoin, urlsplit

import aiohttp

from .bounded_http import read_bounded_response
from .civitai_image_codec import valid_preview_redirect, valid_preview_url
from .provider_credentials import civitai_authorization
from .provider_tls import provider_tls_context


class CivitaiResourceMissing(ValueError):
    """Distinguish an explicit provider 404 from invalid or unavailable metadata."""


def approved_civitai_api_url(url: str) -> bool:
    """Admit only reviewed HTTPS API routes, including provider-owned pagination."""
    try:
        parsed = urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname == "civitai.com"
            and parsed.port in (None, 443)
            and not parsed.username
            and not parsed.password
            and not parsed.fragment
            and re.fullmatch(
                r"/api/v1/(?:enums|models(?:/[1-9][0-9]{0,15})?|images|"
                r"model-versions/(?:by-hash/[a-f0-9]{64}|mini/[1-9][0-9]{0,15}|[1-9][0-9]{0,15}))",
                parsed.path,
            )
            is not None
        )
    except ValueError:
        return False


class CivitaiTransport:
    """Keep one provider request budget while excluding credentials from public-access and CDN checks."""

    def __init__(self, api_key: Callable[[], str | None] = lambda: None) -> None:
        """Bind the server credential owner without retaining plaintext credential values."""
        self._api_key = api_key
        self._gate = asyncio.Semaphore(2)
        self._retry_at = 0.0

    async def read_json(self, url: str, *, authenticated: bool = True) -> object:
        """Bound JSON requests and reject unsafe pagination before sending any credentials."""
        if not approved_civitai_api_url(url):
            raise ValueError("The CivitAI API URL is not approved.")
        headers = {
            "Accept": "application/json",
            "User-Agent": "SugarCubes/0.15 model-library",
        }
        if authenticated:
            headers.update(civitai_authorization(self._api_key))
        async with (
            self._gate,
            aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=20),
                headers=headers,
                connector=aiohttp.TCPConnector(ssl=provider_tls_context()),
            ) as session,
        ):
            self._check_cooldown()
            async with session.get(url, allow_redirects=False) as response:
                if response.status == 404:
                    raise CivitaiResourceMissing("CivitAI did not find this resource.")
                if response.status != 200:
                    self._record_cooldown(response)
                    raise ValueError(
                        f"CivitAI is unavailable (HTTP {response.status})."
                    )
                payload: object = json.loads(
                    await read_bounded_response(response, 4 * 1024 * 1024)
                )
                return payload

    async def read_preview(self, url: str) -> bytes:
        """Fetch bounded visual media through reviewed CDN redirects without API credentials."""
        if not valid_preview_url(url):
            raise ValueError("The provider image URL is not approved.")
        async with (
            self._gate,
            aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=20),
                connector=aiohttp.TCPConnector(ssl=provider_tls_context()),
            ) as session,
        ):
            self._check_cooldown()
            for _ in range(4):
                async with session.get(url, allow_redirects=False) as response:
                    if response.status in (301, 302, 303, 307, 308):
                        location = response.headers.get("Location", "")
                        next_url = urljoin(url, location)
                        if not location or not valid_preview_redirect(next_url):
                            raise ValueError(
                                "The provider image redirected outside reviewed origins."
                            )
                        url = next_url
                        continue
                    if response.status != 200 or response.content_type not in (
                        "image/png",
                        "image/jpeg",
                        "image/webp",
                        "image/gif",
                        "video/mp4",
                        "video/webm",
                    ):
                        self._record_cooldown(response)
                        raise ValueError(
                            "The provider image is unavailable or has an unsupported format."
                        )
                    return await read_bounded_response(response, 20 * 1024 * 1024)
            raise ValueError("The provider image exceeded its redirect limit.")

    def _check_cooldown(self) -> None:
        """Avoid retry storms shared across metadata, history, previews, and discovery."""
        if self._retry_at > time.monotonic():
            raise ValueError(
                "CivitAI is temporarily unavailable. Try again after its cooldown."
            )

    def _record_cooldown(self, response: aiohttp.ClientResponse) -> None:
        """Bound provider-supplied retry delays without blocking the event loop."""
        if response.status == 429:
            hint = response.headers.get("Retry-After", "")
            seconds = min(300, max(1, int(hint))) if hint.isdigit() else 60
            self._retry_at = time.monotonic() + seconds
        elif response.status >= 500:
            self._retry_at = time.monotonic() + 10
