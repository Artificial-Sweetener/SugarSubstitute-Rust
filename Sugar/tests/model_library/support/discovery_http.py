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
"""Supply deterministic external suggestion and image boundaries for real HTTP proofs."""

from __future__ import annotations

from dataclasses import replace
from sugarcubes.model_library.application.download_authorization import (
    ModelDownloadAuthorization,
)
from tests.model_library.support.download_access import FixtureDownloadAccess
from sugarcubes.model_library.infrastructure.provider_credentials import civitai_api_key

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

import asyncio
from collections.abc import Callable
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image

from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionReference,
)
from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.domain.provider_releases import (
    InstalledModelArtifact,
    ModelArtifactOffer,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)

CONTENT = b"Small inert discovery fixture. Never load or execute.\n"
DIGEST = hashlib.sha256(CONTENT).hexdigest()


class DiscoveryProviderFixture:
    """Replace remote source IO while keeping reviewed membership, jobs, and publication real."""

    def __init__(
        self,
        root: Path,
        provider_id: str,
        access: ModelDownloadAccess = ModelDownloadAccess.PUBLIC,
    ) -> None:
        """Track source choice and permit controlled offline or cancelled operations."""
        self.root = root
        self.provider_id = provider_id
        self.access = access
        self.installs: list[ModelSuggestionOffer] = []
        self.started = asyncio.Event()
        self.hold: asyncio.Event | None = None
        self.offline = False

    def supports(self, context: ModelSuggestionContext) -> bool:
        """Expose the single reference role used by this deterministic boundary."""
        return context.artifact_kind == "upscale_models"

    def browse_url(self, context: ModelSuggestionContext) -> str:
        """Return a public source page without file destinations."""
        return f"https://{self.provider_id}.info/"

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str],
    ) -> tuple[ModelSuggestion, ...]:
        """Return one owned exact artifact, with public and installed exclusion applied at source."""
        self.started.set()
        if self.hold is not None:
            await self.hold.wait()
        if self.offline:
            raise OSError("Deterministic provider offline")
        if DIGEST in excluded_sha256 or (
            access_policy is ModelSuggestionAccessPolicy.PUBLIC_ONLY
            and self.access is ModelDownloadAccess.API_KEY_REQUIRED
        ):
            return ()
        offer = ModelSuggestionOffer(
            ModelSuggestionReference(self.provider_id, self.provider_id, "model", "v1"),
            replace(
                ModelArtifactOffer(
                    "file",
                    "fixture.pth",
                    DIGEST,
                    len(CONTENT),
                    "https://github.com/fixture/fixture.pth",
                    "upscale_models",
                    self.provider_id,
                ),
                access=self.access,
            ),
            f"https://{self.provider_id}.info/models/fixture",
            "https://image.civitai.com/fixture/image.png",
            1,
        )
        return (
            ModelSuggestion(context, "<b>Fixture</b>", "v1", None, DIGEST, (offer,)),
        )

    async def acquire(
        self,
        suggestion: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Publish only the tiny reviewed fixture, respecting cancellation before the write."""
        assert offer.reference.provider_id == self.provider_id
        assert offer in suggestion.offers
        self.started.set()
        if self.hold is not None:
            await self.hold.wait()
        await ModelDownloadAuthorization(
            FixtureDownloadAccess(offer.artifact.access), civitai_api_key
        ).require(offer.artifact)
        self.installs.append(offer)
        with (self.root / offer.artifact.file_name).open("xb") as stream:
            stream.write(CONTENT)
        if progress is not None:
            progress(len(CONTENT), len(CONTENT))
        return InstalledModelArtifact(offer.artifact.file_name, offer.artifact)


class DiscoveryPreviewFixture:
    """Keep image transport deterministic while exercising production thumbnail preparation."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Generate a small real image instead of a model or remote network operation."""
        self.preparer = ModelThumbnailPreparer(assets, origin="civitai")
        self.urls: list[str] = []
        stream = BytesIO()
        Image.new("RGB", (64, 96), "navy").save(stream, format="PNG")
        self.content = stream.getvalue()

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Record the reviewed URL and publish actual shared image variants."""
        self.urls.append(url)
        return self.preparer.prepare(self.content)

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Use the same real asset checks as production provider previews."""
        return self.preparer.available(thumbnails)
