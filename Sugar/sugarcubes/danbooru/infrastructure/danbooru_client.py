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
"""Ask Danbooru's public JSON API for posts, as SugarSubstitute's Python application asked it.

Every request has a 15 second limit and a bounded response size. A 404 means the post does not
exist; any other failure means Danbooru is unavailable; a reply of the wrong shape is invalid.
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from functools import lru_cache
import json
import logging
import ssl

import aiohttp
import certifi

from ..domain.lookup import Lookup, LookupStatus
from ..domain.posts import DanbooruPost, parse_post

_LOGGER = logging.getLogger(__name__)
DANBOORU_URL = "https://danbooru.donmai.us"
_JSON_LIMIT = 4 * 1024 * 1024


@lru_cache(maxsize=1)
def _tls() -> ssl.SSLContext:
    """Verify Danbooru against the Mozilla trust bundle, whatever the machine's own store holds."""
    return ssl.create_default_context(cafile=certifi.where())


class DanbooruClient:
    """One HTTP session to Danbooru, opened on first use and closed with the host."""

    def __init__(
        self,
        base_url: str = DANBOORU_URL,
        *,
        user_agent: str = "SugarCubes",
        session_factory: Callable[[], aiohttp.ClientSession] | None = None,
    ) -> None:
        """Bind the API origin; tests point it at a local server."""
        self._base = base_url.rstrip("/")
        self._user_agent = user_agent
        self._factory = session_factory
        self._session: aiohttp.ClientSession | None = None

    async def post(self, post_id: int) -> Lookup[DanbooruPost]:
        """One post by its id."""
        return self._parsed(await self._json(f"/posts/{post_id}.json", {}))

    async def post_by_md5(self, md5: str) -> Lookup[DanbooruPost]:
        """The post whose image has this md5."""
        return self._parsed(await self._json("/posts.json", {"md5": md5}))

    async def close(self) -> None:
        """Release the session."""
        if self._session is not None:
            await self._session.close()
            self._session = None

    def _client(self) -> aiohttp.ClientSession:
        """The open session, made on first use."""
        if self._session is None or self._session.closed:
            self._session = (
                self._factory()
                if self._factory
                else aiohttp.ClientSession(
                    timeout=aiohttp.ClientTimeout(total=15),
                    headers={"User-Agent": self._user_agent},
                    connector=aiohttp.TCPConnector(ssl=_tls()),
                )
            )
        return self._session

    async def _json(self, path: str, params: dict[str, str]) -> Lookup[object]:
        """One GET of Danbooru's JSON API."""
        try:
            async with self._client().get(
                self._base + path,
                params=params,
                headers={"Accept": "application/json"},
            ) as response:
                if response.status == 404:
                    return Lookup(LookupStatus.NOT_FOUND)
                if response.status != 200:
                    return Lookup(LookupStatus.UNAVAILABLE)
                body = await self._read(response)
        except (aiohttp.ClientError, asyncio.TimeoutError, ValueError):
            _LOGGER.warning("Danbooru request failed: %s", path, exc_info=True)
            return Lookup(LookupStatus.UNAVAILABLE)
        try:
            return Lookup(LookupStatus.FOUND, json.loads(body))
        except ValueError:
            return Lookup(LookupStatus.UNAVAILABLE)

    @staticmethod
    async def _read(response: aiohttp.ClientResponse) -> bytes:
        """The body, refused when it exceeds the size a post can have."""
        result = bytearray()
        async for block in response.content.iter_chunked(64 * 1024):
            if len(result) + len(block) > _JSON_LIMIT:
                raise ValueError("The Danbooru response exceeds the supported size.")
            result.extend(block)
        return bytes(result)

    @staticmethod
    def _parsed(answer: Lookup[object]) -> Lookup[DanbooruPost]:
        """A found reply read as a post; one of the wrong shape is invalid."""
        if answer.status is not LookupStatus.FOUND:
            return Lookup(answer.status)
        try:
            return Lookup(LookupStatus.FOUND, parse_post(answer.value))
        except ValueError:
            return Lookup(LookupStatus.INVALID)
