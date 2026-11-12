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
"""Own verified same-type, same-base version chronology independently of transport and UI."""

from __future__ import annotations

from dataclasses import replace

from .models import ModelCatalogEntry, ModelMetadata
from .model_updates import ModelUsageRecord
from .artifact_eligibility import (
    matches_civitai_kind,
    same_base_family,
    compatible_civitai_artifact_role,
)
from .provider_releases import ModelProviderRelease


def recorded_release_entry(
    current: ModelCatalogEntry, usage: ModelUsageRecord
) -> ModelCatalogEntry | None:
    """Bind retained provider evidence only to the exact currently installed artifact.

    Usage survives disposable metadata refreshes. Reuse its identity for update
    checks without persisting synthesized metadata or accepting contradictory
    current associations. A changed file, category or known association needs a
    fresh dispatch before it can participate in the recorded family.
    """
    if (
        current.evidence.location_id != usage.location_id
        or current.evidence.kind != usage.kind
        or current.sha256 != usage.sha256
        or not usage.model_id
        or not usage.version_id
        or not usage.base_model.strip()
    ):
        return None
    metadata = current.metadata
    if metadata is not None:
        if metadata.base_model.strip() and not same_base_family(
            metadata.base_model, usage.base_model
        ):
            return None
        if metadata.provider == "civitai" and (
            (
                metadata.provider_model_id
                and metadata.provider_model_id != usage.model_id
            )
            or (
                metadata.provider_version_id
                and metadata.provider_version_id != usage.version_id
            )
        ):
            return None
    return replace(
        current,
        metadata=replace(
            metadata or ModelMetadata(current.evidence.value),
            provider="civitai",
            provider_model_id=usage.model_id,
            provider_version_id=usage.version_id,
            base_model=usage.base_model,
        ),
    )


def compatible_release_family(
    current: ModelCatalogEntry, releases: tuple[ModelProviderRelease, ...]
) -> tuple[ModelProviderRelease, ...]:
    """Mirror Substitute's observed-current, oldest-to-newest compatibility rule.

    Provider order is newest first; numeric IDs and optional dates do not
    establish chronology. Unknown bases or a missing eligible installed version
    cannot establish a family. Keep only eligible artifacts in that family so
    acquisition consumes the same evidence as exploration.
    """
    metadata = current.metadata
    if metadata is None or not metadata.base_model.strip():
        return ()
    family = tuple(
        replace(release, offers=offers)
        for release in reversed(releases)
        if release.version.provider == metadata.provider
        and release.version.model_id == metadata.provider_model_id
        and matches_civitai_kind(release.version.model_type, current.evidence.kind)
        and same_base_family(release.version.base_model, metadata.base_model)
        and (
            offers := tuple(
                replace(offer, kind=current.evidence.kind)
                for offer in release.offers
                if compatible_civitai_artifact_role(
                    release.version.model_type, offer.kind, current.evidence.kind
                )
            )
        )
    )
    if not any(
        release.version.version_id == metadata.provider_version_id for release in family
    ):
        return ()
    return family
