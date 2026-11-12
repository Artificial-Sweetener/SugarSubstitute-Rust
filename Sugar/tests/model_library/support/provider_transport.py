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
"""Model bounded provider transport responses for isolated metadata and acquisition proofs."""

from __future__ import annotations

from collections.abc import AsyncIterator
import aiohttp


class TransportBody:
    """Replace only the remote stream boundary with deterministic bytes."""

    def __init__(self, content: bytes) -> None:
        """Keep one transport payload for a fixture response."""
        self._content = content

    async def iter_chunked(self, size: int) -> AsyncIterator[bytes]:
        """Yield the same bounded blocks as an HTTP content stream."""
        for offset in range(0, len(self._content), size):
            yield self._content[offset : offset + size]


class TransportResponse:
    """Model HTTP response data without exercising private provider methods."""

    def __init__(
        self, status: int, content: bytes, content_type: str = "application/json"
    ) -> None:
        """Prepare one response including a bounded retry hint."""
        self.status = status
        self.content = TransportBody(content)
        self.content_type = content_type
        self.headers = {"Retry-After": "2"}

    async def __aenter__(self) -> TransportResponse:
        """Enter the response lifetime."""
        return self

    async def __aexit__(
        self, exception_type: object, exception: object, traceback: object
    ) -> None:
        """Release fixture transport state."""


class TransportSession:
    """Record public transport calls independently of metadata policy."""

    def __init__(self, response: TransportResponse, expected_timeout: int = 20) -> None:
        """Bind the response selected by this fixture."""
        self.response = response
        self.requests: list[tuple[str, bool]] = []
        self.headers: list[dict[str, str]] = []
        self._expected_timeout = expected_timeout
        self._connector: aiohttp.BaseConnector | None = None

    def factory(
        self,
        *,
        timeout: aiohttp.ClientTimeout,
        headers: dict[str, str] | None = None,
        connector: aiohttp.BaseConnector | None = None,
    ) -> TransportSession:
        """Observe timeout and session headers at the external adapter seam."""
        assert timeout.total == self._expected_timeout
        self._connector = connector
        self.headers.append(dict(headers or {}))
        return self

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Record redirection policy before returning an external fixture response."""
        self.requests.append((url, allow_redirects))
        if headers is not None:
            self.headers.append(headers)
        return self.response

    async def __aenter__(self) -> TransportSession:
        """Enter the session lifetime."""
        return self

    async def __aexit__(
        self, exception_type: object, exception: object, traceback: object
    ) -> None:
        """Release the fixture session."""
        if self._connector is not None:
            await self._connector.close()
