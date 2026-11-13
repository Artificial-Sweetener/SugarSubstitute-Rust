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
"""Narrow family-discovery responses through the shared role, artifact, and image policies."""

from __future__ import annotations

from dataclasses import replace

from ..domain.provider_releases import ModelDownloadAccess

from ..domain.artifact_eligibility import matches_civitai_kind
from ..domain.model_families import ModelDiscoveryFamily
from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionReference,
)
from ..domain.thumbnail_safety import CivitaiThumbnailSafety
from .civitai_artifact_codec import parse_civitai_artifact
from .civitai_image_codec import select_civitai_thumbnail


def parse_family_candidates(
    payload: object,
    context: ModelSuggestionContext,
    family: ModelDiscoveryFamily,
    rank: int,
    thumbnail_policy: CivitaiThumbnailSafety,
) -> tuple[ModelSuggestion, ...]:
    """Preserve version order while requiring the exact recommendation base and scanned model artifact."""
    if not isinstance(payload, dict):
        return ()
    model_id, name = provider_identity(payload.get("id")), provider_text(
        payload.get("name")
    )
    model_type, versions = payload.get("type"), payload.get("modelVersions")
    if (
        model_id is None
        or name is None
        or model_type != family.model_type
        or not matches_civitai_kind(family.model_type, context.artifact_kind)
        or payload.get("nsfw") is not False
        or payload.get("mode") is not None
        or not isinstance(versions, list)
        or len(versions) > 500
    ):
        return ()
    creator = payload.get("creator")
    username = (
        provider_text(creator.get("username")) if isinstance(creator, dict) else None
    )
    candidates: list[ModelSuggestion] = []
    for version in versions:
        if not isinstance(version, dict):
            continue
        version_id = provider_identity(version.get("id"))
        version_name = provider_text(version.get("name"))
        availability = provider_text(version.get("availability"))
        files = version.get("files")
        if (
            version_id is None
            or version_name is None
            or version.get("baseModel") != family.recommendation_base
            or availability is not None
            and availability.casefold() != "public"
            or not isinstance(files, list)
            or len(files) > 100
        ):
            continue
        artifact = next(
            (
                offer
                for raw in sorted(
                    files,
                    key=lambda item: not (
                        isinstance(item, dict) and item.get("primary") is True
                    ),
                )
                for offer in (parse_civitai_artifact(raw, context.artifact_kind),)
                if offer is not None
            ),
            None,
        )
        if artifact is None:
            continue
        thumbnail = select_civitai_thumbnail(
            version.get("images"), thumbnail_policy, discovery_portrait=True
        )
        candidates.append(
            ModelSuggestion(
                context,
                name,
                version_name,
                username,
                artifact.sha256,
                (
                    ModelSuggestionOffer(
                        ModelSuggestionReference(
                            "civitai",
                            "CivitAI",
                            model_id,
                            version_id,
                            thumbnail.image_id if thumbnail else None,
                        ),
                        replace(artifact, access=ModelDownloadAccess.API_KEY_REQUIRED),
                        f"https://civitai.com/models/{model_id}?modelVersionId={version_id}",
                        thumbnail.url if thumbnail else None,
                        rank,
                    ),
                ),
            )
        )
    return tuple(candidates)


def provider_identity(value: object) -> str | None:
    """Reject malformed IDs before constructing exact provider routes."""
    return (
        str(value)
        if isinstance(value, int) and not isinstance(value, bool) and 0 < value < 10**16
        else None
    )


def provider_text(value: object) -> str | None:
    """Keep nonempty provider labels literal without coercing dynamic objects."""
    return value.strip() if isinstance(value, str) and value.strip() else None
