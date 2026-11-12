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
"""Validate CivitAI history and scanned model artifacts at the network boundary."""

from __future__ import annotations

from ..domain.provider_releases import ModelProviderRelease
from .civitai_codec import parse_civitai_version
from ..domain.artifact_eligibility import civitai_model_kind
from .civitai_artifact_codec import parse_civitai_artifact
from ..domain.thumbnail_safety import CivitaiThumbnailSafety


def parse_civitai_releases(
    payload: object,
    expected_model_id: str,
    thumbnail_policy: CivitaiThumbnailSafety = CivitaiThumbnailSafety.SFW_ONLY,
) -> tuple[ModelProviderRelease, ...]:
    """Require parent identity and retain provider history without fabricating download permission."""
    if not isinstance(payload, dict) or str(payload.get("id")) != expected_model_id:
        raise ValueError("CivitAI returned an unexpected model history identity.")
    title, versions = payload.get("name"), payload.get("modelVersions")
    if (
        not isinstance(title, str)
        or not isinstance(versions, list)
        or len(versions) > 500
    ):
        raise ValueError("CivitAI returned invalid model release history.")
    model_type = payload.get("type")
    kind = civitai_model_kind(model_type) if isinstance(model_type, str) else None
    releases: list[ModelProviderRelease] = []
    for raw in versions:
        if not isinstance(raw, dict):
            raise ValueError("CivitAI returned invalid version history.")
        version = parse_civitai_version(
            {
                **raw,
                "model": {
                    "name": title,
                    "type": model_type,
                    "tags": payload.get("tags", []),
                    "nsfw": payload.get("nsfw"),
                    "nsfwLevel": payload.get("nsfwLevel"),
                },
                "modelId": int(expected_model_id),
            },
            thumbnail_policy,
        )
        public = (
            raw.get("availability", "Public") == "Public"
            and raw.get("status", "Published") == "Published"
            and not payload.get("mode")
        )
        files: object = raw.get("files", [])
        if not isinstance(files, list) or len(files) > 100:
            raise ValueError("CivitAI returned invalid version artifacts.")
        offers = tuple(
            offer
            for file in sorted(
                files,
                key=lambda item: not (
                    isinstance(item, dict) and item.get("primary") is True
                ),
            )
            for offer in (
                parse_civitai_artifact(file, kind) if public and kind else None,
            )
            if offer is not None
        )
        published = raw.get("publishedAt", raw.get("createdAt", ""))
        releases.append(
            ModelProviderRelease(
                version, published if isinstance(published, str) else "", offers
            )
        )
    return tuple(releases)
