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
"""Narrow CivitAI responses into safe provider-neutral metadata evidence."""

from __future__ import annotations


from ..domain.provider_models import ProviderModelVersion
from ..domain.thumbnail_safety import CivitaiThumbnailSafety
from .civitai_image_codec import select_civitai_thumbnail
from .model_content_codec import optional_nsfw, optional_nsfw_level


def parse_civitai_version(
    payload: object,
    thumbnail_policy: CivitaiThumbnailSafety = CivitaiThumbnailSafety.SFW_ONLY,
) -> ProviderModelVersion:
    """Require provider identity and select an image through the shared safety policy."""
    if not isinstance(payload, dict) or not isinstance(payload.get("model"), dict):
        raise ValueError("CivitAI returned invalid model-version metadata.")
    model: dict[object, object] = payload["model"]
    version_id = _identity(payload.get("id"))
    model_id = _identity(payload.get("modelId"))
    title = _required_text(model.get("name"))
    version = _required_text(payload.get("name"))
    words: object = payload.get("trainedWords", [])
    if not isinstance(words, list) or any(not isinstance(word, str) for word in words):
        raise ValueError("CivitAI returned invalid trained words.")
    thumbnail = select_civitai_thumbnail(payload.get("images"), thumbnail_policy)
    tags: object = model.get("tags", [])
    if not isinstance(tags, list) or any(not isinstance(tag, str) for tag in tags):
        raise ValueError("CivitAI returned invalid model tags.")
    return ProviderModelVersion(
        provider="civitai",
        model_id=model_id,
        version_id=version_id,
        title=title,
        version=version,
        base_model=_text(payload.get("baseModel")),
        page_url=f"https://civitai.com/models/{model_id}?modelVersionId={version_id}",
        description=_text(payload.get("description")),
        trained_words=tuple(word for word in words if isinstance(word, str)),
        preview_url=thumbnail.url if thumbnail else None,
        model_type=_text(model.get("type")),
        tags=tuple(tag for tag in tags if isinstance(tag, str)),
        provider_nsfw=optional_nsfw(model.get("nsfw")),
        provider_nsfw_level=optional_nsfw_level(model.get("nsfwLevel")),
    )


def _identity(value: object) -> str:
    """Reject booleans and malformed provider IDs before using them in URLs."""
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError("CivitAI returned an invalid model identity.")
    return str(value)


def _required_text(value: object) -> str:
    """Require meaningful provider labels without coercing arbitrary response values."""
    text = _text(value)
    if not text:
        raise ValueError("CivitAI returned an empty model label.")
    return text


def _text(value: object) -> str:
    """Preserve literal provider text for safe downstream rendering."""
    return value if isinstance(value, str) else ""
