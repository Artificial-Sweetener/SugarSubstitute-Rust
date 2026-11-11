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
"""Stream verified model artifacts through reviewed public download origins."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
import ipaddress
import logging
import socket
from typing import TypeVar
from urllib.parse import urljoin, urlsplit

import aiohttp
from .provider_tls import provider_tls_context
from aiohttp.abc import ResolveResult

from ..application.ports import ModelRoots
from ..application.download_authorization import ModelDownloadAuthorization
from ..domain.provider_releases import ModelArtifactOffer
from .artifact_files import ModelArtifactFile
from ..domain.artifact_transport import (
    approved_artifact_start,
    approved_artifact_redirect,
)
from .provider_credentials import civitai_authorization

_LOGGER = logging.getLogger(__name__)
_T = TypeVar("_T")


class PublicModelDownloadResolver(aiohttp.abc.AbstractResolver):
    """Reject nonpublic DNS results before a signed provider redirect can become local access."""

    def __init__(self) -> None:
        """Use the platform resolver behind an explicit public-address boundary."""
        self._resolver = aiohttp.resolver.DefaultResolver()

    async def resolve(
        self, host: str, port: int = 0, family: int = socket.AF_INET
    ) -> list[ResolveResult]:
        """Fail closed if any resolved endpoint is private, loopback, reserved, or otherwise nonpublic."""
        results = await self._resolver.resolve(host, port, socket.AddressFamily(family))
        if not results or any(
            not ipaddress.ip_address(result["host"]).is_global for result in results
        ):
            raise ValueError(
                "The model download origin did not resolve to a public address."
            )
        return results

    async def close(self) -> None:
        """Release platform DNS resources when the download session ends."""
        await self._resolver.close()


class ModelArtifactDownloads:
    """Keep download transport, cancellation, and disk lifetime behind the acquisition port."""

    def __init__(
        self,
        roots: ModelRoots,
        credentials: Callable[[], str | None],
        authorization: ModelDownloadAuthorization,
    ) -> None:
        """Bind approved host roots and a server-owned credential source."""
        self._roots = roots
        self._credentials = credentials
        self._authorization = authorization
        self._gate = asyncio.Semaphore(2)

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Acquire one validated artifact without replacing any installed model file."""
        return await self.install_to(offer, progress=progress)

    async def install_to(
        self,
        offer: ModelArtifactOffer,
        relative_path: str | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Use one transport for browser selections and preserved desktop destination preferences."""
        if not approved_artifact_start(offer.download_url, offer.provider):
            raise ValueError("The model download route is not approved.")
        file = ModelArtifactFile(
            self._roots.install_root(offer.kind),
            offer,
            relative_path,
        )
        async with self._gate:
            try:
                existing = await _disk(file.reuse)
                if existing is not None:
                    return existing
                await self._authorization.require(offer)
                await _disk(file.begin)
                await self._transfer(offer, file, progress)
                return await _disk(file.publish)
            except asyncio.CancelledError:
                if file.committed_name is not None:
                    return file.committed_name
                raise
            finally:
                await _disk(file.abort)

    async def _transfer(
        self,
        offer: ModelArtifactOffer,
        file: ModelArtifactFile,
        progress: Callable[[int, int], None] | None = None,
    ) -> None:
        """Validate every redirect, strip cross-origin authentication, and bound transfer time."""
        url = offer.download_url
        resolver = PublicModelDownloadResolver()
        try:
            async with aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=7200, connect=20, sock_read=60),
                connector=aiohttp.TCPConnector(
                    resolver=resolver, ssl=provider_tls_context()
                ),
            ) as session:
                for _ in range(6):
                    if not approved_artifact_redirect(url, offer.provider):
                        raise ValueError(
                            "The model download redirected outside reviewed origins."
                        )
                    headers = (
                        civitai_authorization(self._credentials)
                        if offer.provider == "civitai"
                        and urlsplit(url).hostname == "civitai.com"
                        else {}
                    )
                    async with session.get(
                        url, headers=headers, allow_redirects=False
                    ) as response:
                        if response.status in (301, 302, 303, 307, 308):
                            location = response.headers.get("Location")
                            if not location:
                                raise ValueError(
                                    "The model provider returned an invalid redirect."
                                )
                            url = urljoin(url, location)
                            continue
                        if response.status in (401, 403):
                            raise ValueError(
                                "CivitAI rejected download access. Check your API key and account permissions."
                            )
                        if response.status != 200:
                            raise ValueError(
                                f"The model download is unavailable (HTTP {response.status})."
                            )
                        response_size = response.headers.get("Content-Length")
                        if response_size is not None and (
                            len(response_size) > 20
                            or not response_size.isascii()
                            or not response_size.isdecimal()
                            or int(response_size) != offer.size_bytes
                        ):
                            raise ValueError(
                                "The model response size does not match provider metadata."
                            )
                        async for block in response.content.iter_chunked(1024 * 1024):
                            await _disk(lambda: file.append(block))
                            if progress is not None:
                                progress(file.bytes_written, offer.size_bytes)
                        return
                raise ValueError("The model download exceeded its redirect limit.")
        except (aiohttp.ClientError, TimeoutError) as error:
            # Transport exceptions can contain signed URLs; publish the failure type without exposing query credentials.
            _LOGGER.warning(
                "Model download transport failed",
                extra={"failure_type": type(error).__name__},
            )
            raise ValueError(
                "The model transfer failed. Check the connection and try again."
            ) from None
        finally:
            await resolver.close()


async def _disk(operation: Callable[[], _T]) -> _T:
    """Drain an in-flight disk operation before cancellation can close or remove its temporary file."""
    task = asyncio.create_task(asyncio.to_thread(operation))
    try:
        return await asyncio.shield(task)
    except asyncio.CancelledError:
        await task
        raise
