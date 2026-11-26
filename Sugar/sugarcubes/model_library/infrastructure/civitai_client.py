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
"""Adapt shared CivitAI transport to metadata identity and negative-cache contracts."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import replace
import asyncio
import logging
import aiohttp
import re
import time

from ..domain.models import PreviewCandidate
from ..domain.provider_models import ProviderModelVersion
from ..domain.provider_releases import ModelProviderRelease
from ..domain.thumbnail_safety import CivitaiThumbnailSafety
from ..application.metadata_activity import ModelMetadataActivity
from .civitai_download_access import CivitaiDownloadAccess
from .civitai_codec import parse_civitai_version
from .civitai_image_codec import civitai_preview_candidates
from .civitai_release_codec import parse_civitai_releases
from .civitai_transport import CivitaiResourceMissing, CivitaiTransport
from .metadata_codec import normalized_hash
from .model_content_codec import optional_nsfw, optional_nsfw_level

_LOGGER = logging.getLogger(__name__)


class CivitaiModelProvider:
    """Resolve installed-model metadata and history through one shared provider transport."""

    def __init__(
        self,
        api_key: Callable[[], str | None] = lambda: None,
        *,
        transport: CivitaiTransport | None = None,
        download_access: CivitaiDownloadAccess | None = None,
        thumbnail_policy: Callable[
            [], CivitaiThumbnailSafety
        ] = lambda: CivitaiThumbnailSafety.SFW_ONLY,
        lookup_enabled: Callable[[], bool] = lambda: True,
        requested_lookup_enabled: Callable[[], bool] = lambda: True,
        activity: ModelMetadataActivity | None = None,
    ) -> None:
        """Bind the transport shared with discovery and retain process-only missing-hash evidence."""
        self._transport = transport or CivitaiTransport(api_key)
        self._download_access = download_access or CivitaiDownloadAccess(
            self._transport
        )
        self._thumbnail_policy = thumbnail_policy
        self._lookup_enabled = lookup_enabled
        self._requested_lookup_enabled = requested_lookup_enabled
        self._missing: dict[str, float] = {}
        self._activity = activity or ModelMetadataActivity()

    async def lookup(
        self,
        sha256: str,
        weights_hash: Callable[[], Awaitable[str | None]],
        *,
        ask_again: bool = False,
    ) -> ProviderModelVersion | None:
        """Resolve verified content identity while caching only explicit provider misses.

        A file whose full hash CivitAI does not know is asked for again by the hash of its
        weights alone: an uploader who rewrites a file's header gives it a new full hash while
        its weights, and so its identity, stay the same. ``ask_again`` forgets the remembered
        misses of this file first, so a lookup a person asked for always reaches CivitAI.
        """
        if not self._lookup_enabled():
            return None
        if ask_again:
            self._missing.pop(normalized_hash(sha256), None)
        version = await self._version_by_hash(sha256)
        if version is not None:
            return version
        weights = await weights_hash()
        if weights is None:
            return None
        if ask_again:
            self._missing.pop(normalized_hash(weights)[:12], None)
        return await self._version_by_weights(weights)

    async def _version_by_weights(self, weights: str) -> ProviderModelVersion | None:
        """Accept a weights-hash answer only when one of its files states that same hash."""
        short = normalized_hash(weights)[:12]
        if self._missing.get(short, 0) > time.monotonic():
            return None
        async with self._activity.work():
            try:
                payload = await self._transport.read_json(
                    f"https://civitai.com/api/v1/model-versions/by-hash/{short}"
                )
            except CivitaiResourceMissing:
                self._remember_missing(short)
                return None
        if not _states_weights_hash(payload, short):
            self._remember_missing(short)
            return None
        return parse_civitai_version(payload, self._thumbnail_policy())

    async def requested_version(self, sha256: str) -> ProviderModelVersion | None:
        """Apply explicit missing-model policy without enabling automatic metadata enrichment."""
        if not self._requested_lookup_enabled():
            return None
        return await self._version_by_hash(sha256)

    async def _version_by_hash(self, sha256: str) -> ProviderModelVersion | None:
        """Share validated identity and missing-hash evidence across both lookup purposes."""
        payload = await self.raw_version_by_hash(sha256)
        version = (
            parse_civitai_version(payload, self._thumbnail_policy())
            if payload is not None
            else None
        )
        if (
            version is None
            or version.provider_nsfw is None
            or version.provider_nsfw_level is not None
        ):
            return version
        try:
            model = await self._transport.read_json(
                f"https://civitai.com/api/v1/models/{version.model_id}"
            )
            if not isinstance(model, dict) or str(model.get("id")) != version.model_id:
                raise ValueError("CivitAI returned mismatched model classification.")
            return replace(
                version,
                provider_nsfw=optional_nsfw(model.get("nsfw")),
                provider_nsfw_level=optional_nsfw_level(model.get("nsfwLevel")),
            )
        except (ValueError, aiohttp.ClientError, TimeoutError):
            _LOGGER.warning(
                "CivitAI model classification enrichment unavailable",
                extra={"provider_model_id": version.model_id},
                exc_info=True,
            )
            return version

    async def raw_version_by_hash(self, sha256: str) -> object | None:
        """Share missing-hash ownership with discovery while keeping dynamic payloads inside adapters."""
        async with self._activity.work():
            return await self._raw_version_by_hash(sha256)

    async def _raw_version_by_hash(self, sha256: str) -> object | None:
        """Retain confirmed-miss publication through the complete exact lookup lease."""
        fingerprint = normalized_hash(sha256)
        if self._missing.get(fingerprint, 0) > time.monotonic():
            return None
        try:
            payload = await self._transport.read_json(
                f"https://civitai.com/api/v1/model-versions/by-hash/{fingerprint}"
            )
        except CivitaiResourceMissing:
            self._remember_missing(fingerprint)
            return None
        if payload is None:
            raise ValueError("CivitAI returned invalid model-version metadata.")
        return payload

    async def version_previews(self, version_id: str) -> tuple[PreviewCandidate, ...]:
        """Read the images one known version offers; a version CivitAI no longer has offers none."""
        if not re.fullmatch(r"[1-9][0-9]{0,15}", version_id):
            raise ValueError("Invalid CivitAI model version identity.")
        async with self._activity.work():
            try:
                payload = await self._transport.read_json(
                    f"https://civitai.com/api/v1/model-versions/{version_id}"
                )
            except CivitaiResourceMissing:
                return ()
        return civitai_preview_candidates(
            payload.get("images") if isinstance(payload, dict) else None
        )

    def _remember_missing(self, identity: str) -> None:
        """Keep a bounded hour-long memory of hashes the provider does not know."""
        self._missing[identity] = time.monotonic() + 3600
        if len(self._missing) > 1024:
            self._missing.pop(next(iter(self._missing)))

    async def read_preview(self, url: str) -> bytes:
        """Delegate image origin and credential isolation to the transport owner."""
        if self._thumbnail_policy() is CivitaiThumbnailSafety.DISABLED:
            raise ValueError("CivitAI thumbnail downloads are disabled.")
        return await self._transport.read_preview(url)

    def invalidate_missing(self) -> None:
        """Forget confirmed misses only under the runtime's exclusive maintenance lease."""
        self._missing.clear()

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Read validated history without opening or hashing installed model files."""
        if not re.fullmatch(r"[1-9][0-9]{0,15}", model_id):
            raise ValueError("Invalid CivitAI model identity.")
        payload = await self._transport.read_json(
            f"https://civitai.com/api/v1/models/{model_id}"
        )
        releases = parse_civitai_releases(payload, model_id, self._thumbnail_policy())
        return tuple(
            await asyncio.gather(
                *(self._release_access(release) for release in releases)
            )
        )

    async def _release_access(
        self, release: ModelProviderRelease
    ) -> ModelProviderRelease:
        """Attach access to each exact artifact without conflating public listing with anonymous download."""
        if not release.offers:
            return release
        access = await self._download_access.version(release.version.version_id)
        return replace(
            release,
            offers=tuple(replace(offer, access=access) for offer in release.offers),
        )


def _states_weights_hash(payload: object, short: str) -> bool:
    """Confirm a by-hash answer names the asked weights hash on one of its own files."""
    files: object = payload.get("files") if isinstance(payload, dict) else None
    if not isinstance(files, list):
        return False
    for item in files:
        hashes: object = item.get("hashes") if isinstance(item, dict) else None
        stated: object = hashes.get("AutoV3") if isinstance(hashes, dict) else None
        if isinstance(stated, str) and stated.casefold() == short:
            return True
    return False
