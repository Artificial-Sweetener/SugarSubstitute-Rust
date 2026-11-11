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
"""Interpret adjacent model presentation without trusting local content-hash claims."""

from __future__ import annotations

import re

from ..domain.models import ModelMetadata, ThumbnailReference


def parse_local_model_metadata(
    payload: object, fallback: str, thumbnails: tuple[ThumbnailReference, ...]
) -> ModelMetadata:
    """Preserve labels and bounded provider hints while keeping fingerprints independently verified."""
    if not isinstance(payload, dict):
        raise ValueError("Local model sidecar must be a JSON object.")
    model: object = payload.get("model")
    model_title: object = model.get("name") if isinstance(model, dict) else None
    model_id = _identity(payload.get("modelId"))
    version_id = _identity(payload.get("modelVersionId"))
    if model_id and not version_id and isinstance(model, dict):
        version_id = _identity(payload.get("id"))
    words: object = payload.get(
        "trainedWords", payload.get("activationText", payload.get("activation text"))
    )
    trained_words = (
        tuple(item for item in words if isinstance(item, str))
        if isinstance(words, list)
        else ((_text(words),) if _text(words) else ())
    )
    tags: object = payload.get(
        "tags", model.get("tags", []) if isinstance(model, dict) else []
    )
    return ModelMetadata(
        title=_text(payload.get("title")) or _text(model_title) or fallback,
        version=_text(payload.get("version"))
        or (_text(payload.get("name")) if model_title else ""),
        base_model=_text(payload.get("baseModel"))
        or _text(payload.get("sdVersion"))
        or _text(payload.get("sd version")),
        description=_text(payload.get("description")),
        trained_words=trained_words,
        thumbnails=thumbnails,
        provider="civitai" if model_id else "",
        provider_url=f"https://civitai.com/models/{model_id}" if model_id else "",
        provider_model_id=model_id,
        provider_version_id=version_id if model_id else "",
        tags=(
            tuple(tag for tag in tags if isinstance(tag, str))
            if isinstance(tags, list)
            else ()
        ),
    )


def _identity(value: object) -> str:
    """Accept known local ID conventions without letting sidecars supply remote URLs."""
    if isinstance(value, bool) or not isinstance(value, (str, int)):
        return ""
    text = str(value)
    return text if re.fullmatch(r"[1-9][0-9]{0,19}", text) else ""


def _text(value: object) -> str:
    """Retain literal sidecar strings without coercing foreign values."""
    return value.strip() if isinstance(value, str) else ""
