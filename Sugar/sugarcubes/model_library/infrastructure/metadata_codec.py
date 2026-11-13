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
"""Validate portable metadata at the persistent storage boundary."""

from __future__ import annotations

import json
import re

from ..domain.models import ModelMetadata, ThumbnailReference
from .provider_link_codec import provider_link_value
from .model_content_codec import optional_nsfw, optional_nsfw_level


def normalized_hash(value: str) -> str:
    """Reject non-SHA256 keys before they address metadata or asset storage."""
    if re.fullmatch(r"[a-fA-F0-9]{64}", value) is None:
        raise ValueError("Expected a SHA256 model or asset identity.")
    return value.lower()


def encode_metadata(metadata: ModelMetadata) -> str:
    """Serialize a stable, host-neutral metadata record."""
    return json.dumps(
        {
            "title": metadata.title,
            "version": metadata.version,
            "baseModel": metadata.base_model,
            "provider": metadata.provider,
            "providerUrl": metadata.provider_url,
            "providerModelId": metadata.provider_model_id,
            "providerVersionId": metadata.provider_version_id,
            "providerLinks": [link.to_payload() for link in metadata.provider_links],
            "description": metadata.description,
            "trainedWords": list(metadata.trained_words),
            "tags": list(metadata.tags),
            "modelType": metadata.model_type,
            "providerNsfw": metadata.provider_nsfw,
            "providerNsfwLevel": metadata.provider_nsfw_level,
            "thumbnails": [item.to_payload() for item in metadata.thumbnails],
        },
        ensure_ascii=False,
        sort_keys=True,
    )


def decode_metadata(payload: str) -> ModelMetadata:
    """Fail explicitly when persisted metadata violates the catalog schema."""
    value: object = json.loads(payload)
    if not isinstance(value, dict):
        raise ValueError("Model metadata must be an object.")
    words: object = value.get("trainedWords", [])
    tags: object = value.get("tags", [])
    variants: object = value.get("thumbnails", [])
    links: object = value.get("providerLinks", [])
    if not isinstance(words, list) or not all(isinstance(word, str) for word in words):
        raise ValueError("Model trained words must be strings.")
    if not isinstance(tags, list) or not all(isinstance(tag, str) for tag in tags):
        raise ValueError("Model tags must be strings.")
    if not isinstance(variants, list):
        raise ValueError("Model thumbnails must be a list.")
    if not isinstance(links, list) or len(links) > 16:
        raise ValueError("Model provider links must be a bounded list.")
    return ModelMetadata(
        title=_string(value.get("title")),
        version=_string(value.get("version", "")),
        base_model=_string(value.get("baseModel", "")),
        provider=_string(value.get("provider", "")),
        provider_url=_string(value.get("providerUrl", "")),
        provider_model_id=_string(value.get("providerModelId", "")),
        provider_version_id=_string(value.get("providerVersionId", "")),
        description=_string(value.get("description", "")),
        trained_words=tuple(_string(word) for word in words),
        tags=tuple(_string(tag) for tag in tags),
        model_type=_string(value.get("modelType", "")),
        provider_nsfw=optional_nsfw(value.get("providerNsfw")),
        provider_nsfw_level=optional_nsfw_level(value.get("providerNsfwLevel")),
        thumbnails=tuple(_thumbnail(item) for item in variants),
        provider_links=tuple(provider_link_value(item) for item in links),
    )


def _string(value: object) -> str:
    """Narrow dynamic stored text without coercing invalid schema values."""
    if not isinstance(value, str):
        raise ValueError("Expected model metadata text.")
    return value


def _thumbnail(value: object) -> ThumbnailReference:
    """Validate image geometry and opaque asset identity before publication."""
    if not isinstance(value, dict):
        raise ValueError("Thumbnail reference must be an object.")
    width: object = value.get("width")
    height: object = value.get("height")
    role = _string(value.get("role"))
    content_type = _string(value.get("contentType"))
    if (
        not isinstance(width, int)
        or isinstance(width, bool)
        or width <= 0
        or not isinstance(height, int)
        or isinstance(height, bool)
        or height <= 0
        or role not in ("standard", "banner")
        or content_type not in ("image/png", "image/jpeg", "image/webp")
    ):
        raise ValueError("Invalid prepared thumbnail format or geometry.")
    return ThumbnailReference(
        asset_id=normalized_hash(_string(value.get("assetId"))),
        role=role,
        width=width,
        height=height,
        content_type=content_type,
    )
