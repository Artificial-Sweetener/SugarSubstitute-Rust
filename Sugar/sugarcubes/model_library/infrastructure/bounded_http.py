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
"""Bound decoded HTTP bytes before provider payloads reach parsers or storage."""

from __future__ import annotations

import aiohttp


async def read_bounded_response(response: aiohttp.ClientResponse, limit: int) -> bytes:
    """Enforce the decoded size limit even when transport content length is inaccurate."""
    result = bytearray()
    async for block in response.content.iter_chunked(64 * 1024):
        if len(result) + len(block) > limit:
            raise ValueError("The provider response exceeds the supported size limit.")
        result.extend(block)
    return bytes(result)
