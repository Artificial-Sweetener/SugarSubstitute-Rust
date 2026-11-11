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
"""Resolve desktop acquisition recipes against current provider-owned safe offers."""

from __future__ import annotations

from dataclasses import dataclass
from dataclasses import replace
from typing import Protocol

from ..domain.provider_models import ProviderModelVersion
from ..domain.artifact_eligibility import (
    matches_civitai_kind,
    compatible_civitai_artifact_role,
)
from ..domain.provider_releases import ModelArtifactOffer, ModelProviderRelease


class RequestedArtifactProvider(Protocol):
    """Resolve content identities and their provider-owned acquisition evidence."""

    async def requested_version(self, sha256: str) -> ProviderModelVersion | None:
        """Find a provider version by its claimed content identity."""

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return current safe acquisition offers for the identified model."""


@dataclass(frozen=True, slots=True)
class RequestedModelArtifacts:
    """Keep user recipes subordinate to current provider safety and category evidence."""

    provider: RequestedArtifactProvider

    async def offer(self, sha256: str, kind: str) -> ModelArtifactOffer:
        """Revalidate SHA256, model category, and provider scans before any transfer begins."""
        version = await self.provider.requested_version(sha256)
        if version is not None and matches_civitai_kind(version.model_type, kind):
            for release in await self.provider.releases(version.model_id):
                if (
                    release.version.model_id != version.model_id
                    or release.version.version_id != version.version_id
                    or release.version.provider != version.provider
                    or not matches_civitai_kind(release.version.model_type, kind)
                ):
                    continue
                for offer in release.offers:
                    if (
                        offer.sha256 == sha256
                        and compatible_civitai_artifact_role(
                            release.version.model_type, offer.kind, kind
                        )
                        and offer.file_name.lower().endswith(".safetensors")
                    ):
                        return replace(offer, kind=kind)
        raise ValueError(
            "The requested model has no current verified SafeTensor acquisition offer."
        )
