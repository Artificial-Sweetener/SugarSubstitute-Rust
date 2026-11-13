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
"""Validate persisted provider-neutral history before applying freshness or offering files."""

from __future__ import annotations

import json
import re
from datetime import datetime

from ..application.release_history import ReleaseHistorySnapshot
from ..domain.artifact_transport import approved_artifact_start
from ..domain.provider_models import ProviderModelVersion
from ..domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
    ModelDownloadAccess,
)
from .metadata_codec import decode_metadata, encode_metadata, normalized_hash


def encode_history(snapshot: ReleaseHistorySnapshot) -> str:
    """Serialize successful history without including credentials or model paths."""
    return json.dumps(
        {
            "schemaVersion": 1,
            "checkedAt": snapshot.checked_at.isoformat(),
            "releases": [
                {
                    "metadata": encode_metadata(release.version.metadata(())),
                    "previewUrl": release.version.preview_url,
                    "publishedAt": release.published_at,
                    "files": [
                        {
                            **offer.to_payload(),
                            "provider": offer.provider,
                            "downloadUrl": offer.download_url,
                        }
                        for offer in release.offers
                    ],
                }
                for release in snapshot.releases
            ],
        }
    )


def decode_history(payload: str) -> ReleaseHistorySnapshot:
    """Reject invalid timestamps, provider identities or unsafe cached acquisition offers."""
    value: object = json.loads(payload)
    if not isinstance(value, dict) or value.get("schemaVersion") != 1:
        raise ValueError("Invalid release history schema.")
    checked_at = datetime.fromisoformat(_text(value.get("checkedAt")))
    rows: object = value.get("releases")
    if checked_at.tzinfo is None or not isinstance(rows, list) or len(rows) > 500:
        raise ValueError("Invalid release history timestamp or records.")
    return ReleaseHistorySnapshot(checked_at, tuple(_release(row) for row in rows))


def _release(value: object) -> ModelProviderRelease:
    """Reuse catalog metadata validation for provider labels and narrow release-specific fields."""
    if not isinstance(value, dict):
        raise ValueError("Invalid release history record.")
    metadata = decode_metadata(_text(value.get("metadata")))
    for identity in (metadata.provider_model_id, metadata.provider_version_id):
        if not re.fullmatch(r"[1-9][0-9]{0,15}", identity):
            raise ValueError("Invalid release history provider identity.")
    preview: object = value.get("previewUrl")
    files: object = value.get("files")
    if (
        (preview is not None and not isinstance(preview, str))
        or not isinstance(files, list)
        or len(files) > 100
    ):
        raise ValueError("Invalid release preview or artifacts.")
    return ModelProviderRelease(
        ProviderModelVersion(
            metadata.provider,
            metadata.provider_model_id,
            metadata.provider_version_id,
            metadata.title,
            metadata.version,
            metadata.base_model,
            metadata.provider_url,
            metadata.description,
            metadata.trained_words,
            preview,
            metadata.provider_links,
            metadata.model_type,
            metadata.tags,
            metadata.provider_nsfw,
            metadata.provider_nsfw_level,
        ),
        _text(value.get("publishedAt")),
        tuple(_offer(file) for file in files),
    )


def _offer(value: object) -> ModelArtifactOffer:
    """Treat stored download evidence as untrusted until provider origin and bounds are validated."""
    if not isinstance(value, dict):
        raise ValueError("Invalid release artifact.")
    identity = _text(value.get("fileId"))
    size: object = value.get("sizeBytes")
    url, provider = _text(value.get("downloadUrl")), _text(value.get("provider"))
    if (
        not re.fullmatch(r"[1-9][0-9]{0,15}", identity)
        or isinstance(size, bool)
        or not isinstance(size, int)
        or not 0 < size <= 128 * 1024**3
        or not approved_artifact_start(url, provider)
    ):
        raise ValueError("Invalid release artifact identity or transport.")
    return ModelArtifactOffer(
        identity,
        _text(value.get("fileName")),
        normalized_hash(_text(value.get("sha256"))),
        size,
        url,
        _text(value.get("kind")),
        provider,
        ModelDownloadAccess(value.get("access", "unknown")),
    )


def _text(value: object) -> str:
    """Require stored text rather than coercing dynamic values."""
    if not isinstance(value, str):
        raise ValueError("Invalid release history text.")
    return value
