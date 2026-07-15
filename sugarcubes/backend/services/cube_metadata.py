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
"""Normalize cube metadata and authored identity fields at one domain boundary."""

from __future__ import annotations

import re
from typing import Any, Mapping, Optional, Sequence

from ...cube_model import (
    CubeIdentityError,
    derive_source_author_label,
    derive_target_model_from_cube_id,
    normalize_cube_route,
    normalize_supported_models as normalize_supported_model_labels,
    normalize_target_model,
    validate_cube_route_identity,
)
from ..responses import BackendError
from .cube_icon_service import CubeIconError, normalize_icon_metadata

_TITLE_SMALL_WORDS = {
    "a",
    "an",
    "and",
    "as",
    "at",
    "but",
    "by",
    "for",
    "from",
    "in",
    "into",
    "nor",
    "of",
    "on",
    "or",
    "per",
    "the",
    "to",
    "vs",
    "via",
    "with",
}
_TITLE_TECH_TERMS = {
    "ai": "AI",
    "clip": "CLIP",
    "ipadapter": "IPAdapter",
    "lora": "LoRA",
    "sd": "SD",
    "sdxl": "SDXL",
    "ui": "UI",
    "vae": "VAE",
    "xl": "XL",
}
_ASCII_LETTER_RE = re.compile(r"[A-Za-z]")
_ASCII_WORD_RE = re.compile(r"[A-Za-z]+")
_VERSION_TOKEN_RE = re.compile(r"v[0-9]+(?:[A-Za-z0-9.-]*)?$", re.IGNORECASE)


def normalize_metadata_string(value: Any) -> str:
    """Normalize metadata text fields to non-empty strings."""

    if isinstance(value, str):
        trimmed = value.strip()
        if trimmed:
            return trimmed
    return ""


def _mapping_list(value: object) -> list[dict[str, Any]]:
    """Copy a dynamic sequence of mappings into mutable typed payloads."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [dict(entry) for entry in value if isinstance(entry, Mapping)]


def normalize_tag_value(value: object) -> str:
    """Normalize a tag into a stable lowercase slug."""

    if not isinstance(value, str):
        return ""
    tokens = re.findall(r"[0-9a-zA-Z]+", value.lower())
    if not tokens:
        return ""
    if all(len(token) == 1 for token in tokens):
        return "".join(tokens)
    return "-".join(tokens)


def normalize_tags(value: Any) -> list[str]:
    """Coerce metadata tags into a list of normalized strings."""

    if isinstance(value, list):
        tags: list[str] = []
        for item in value:
            if isinstance(item, str):
                cleaned = normalize_tag_value(item)
                if cleaned:
                    tags.append(cleaned)
        return tags
    if isinstance(value, str):
        return [
            part
            for part in (normalize_tag_value(segment) for segment in value.split(","))
            if part
        ]
    return []


def normalize_supported_models(value: Any, *, target_model: str = "") -> list[str]:
    """Coerce supported model metadata through the shared target-model policy."""

    return normalize_supported_model_labels(value, target_model=target_model)


def validate_target_model_matches_cube_id(cube_id: str, target_model: str) -> None:
    """Reject target metadata that disagrees with the cube's repo path."""

    normalized_cube_id = normalize_metadata_string(cube_id)
    normalized_target = normalize_target_model(target_model)
    if not normalized_target or not normalized_cube_id:
        return
    try:
        path_target = derive_target_model_from_cube_id(normalized_cube_id)
    except CubeIdentityError as exc:
        raise BackendError(str(exc), status=400) from exc
    if path_target != normalized_target:
        raise BackendError(
            "metadata.target_model must match the cube id path",
            status=400,
            details={
                "cube_id": normalized_cube_id,
                "target_model": normalized_target,
                "path_target_model": path_target,
            },
        )


def normalize_lineage_payload(value: Any) -> Optional[dict[str, str]]:
    """Normalize lineage metadata into a compact mapping."""

    if not isinstance(value, Mapping):
        return None
    result: dict[str, str] = {}
    for key in (
        "id",
        "name",
        "version",
        "revision_ref",
        "author",
        "author_url",
        "forked_at",
    ):
        normalized = normalize_metadata_string(value.get(key))
        if normalized:
            result[key] = normalized
    return result or None


def derive_source_author_from_identity(
    cube_id: str,
    *,
    source_kind: str,
    owner: str,
    repo: str,
    namespace: str,
) -> str:
    """Return the source-derived author label for browser metadata."""

    if cube_id:
        try:
            return derive_source_author_label(cube_id)
        except CubeIdentityError:
            pass
    if source_kind == "github" and owner and repo:
        return f"{owner}/{repo}"
    if source_kind == "local" and namespace:
        return "local"
    return ""


def normalize_default_alias(value: Any) -> str:
    """Normalize a route-based user-supplied cube default alias."""

    try:
        route = normalize_cube_route(value)
    except CubeIdentityError as exc:
        raise BackendError(str(exc), status=400) from exc
    if not route:
        return ""
    segments = route.split("/")
    if len(segments) > 1:
        try:
            segments[0] = normalize_target_model(segments[0]) or segments[0]
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
    return "/".join(segments)




def normalize_default_alias_title(value: Any) -> str:
    """Normalize an authored cube display title without changing path semantics."""

    if not isinstance(value, str):
        return ""
    cleaned = value.strip()
    if not cleaned:
        return ""
    if "_" in cleaned or not _ASCII_LETTER_RE.search(cleaned):
        return cleaned
    parts = re.split(r"\s+", cleaned)
    last_index = len(parts) - 1
    return " ".join(
        _normalize_title_token(part, index=index, is_last=index == last_index)
        for index, part in enumerate(parts)
    )


def _normalize_title_token(token: str, *, index: int, is_last: bool) -> str:
    """Return one conservative title-cased token for authored cube names."""

    word_match = _ASCII_WORD_RE.search(token)
    if not word_match:
        return token
    word = word_match.group(0)
    word_lower = word.lower()
    if word_lower in _TITLE_TECH_TERMS:
        replacement = _TITLE_TECH_TERMS[word_lower]
    elif index > 0 and not is_last and word_lower in _TITLE_SMALL_WORDS:
        replacement = word_lower
    elif _VERSION_TOKEN_RE.fullmatch(token):
        replacement = token.lower()
        return replacement
    elif _has_mixed_casing_beyond_first_character(word):
        return token
    else:
        replacement = word[:1].upper() + word[1:].lower()
    return f"{token[: word_match.start()]}{replacement}{token[word_match.end() :]}"


def _has_mixed_casing_beyond_first_character(word: str) -> bool:
    """Return whether a token appears intentionally cased already."""

    return any(character.isupper() for character in word[1:]) and any(
        character.islower() for character in word
    )


def normalize_metadata_update(
    payload: Mapping[str, Any], *, cube_id: str = ""
) -> tuple[dict[str, Any], set[str]]:
    """Normalize metadata edit payloads into updates and removals."""

    normalized: dict[str, Any] = {}
    remove_keys: set[str] = set()
    for key in ("author_url",):
        if key not in payload:
            continue
        value = normalize_metadata_string(payload.get(key))
        if value:
            normalized[key] = value
        else:
            remove_keys.add(key)
    if "default_alias" in payload:
        default_alias = normalize_default_alias(payload.get("default_alias"))
        if default_alias:
            if cube_id:
                try:
                    validate_cube_route_identity(cube_id, default_alias)
                except CubeIdentityError as exc:
                    raise BackendError(str(exc), status=400) from exc
            normalized["default_alias"] = default_alias
        else:
            remove_keys.add("default_alias")
    if "tags" in payload:
        tags = normalize_tags(payload.get("tags"))
        if tags:
            normalized["tags"] = tags
        else:
            remove_keys.add("tags")
    if "target_model" in payload:
        try:
            target_model = normalize_target_model(payload.get("target_model"))
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        if target_model:
            validate_target_model_matches_cube_id(cube_id, target_model)
            normalized["target_model"] = target_model
        else:
            if cube_id:
                try:
                    path_target = derive_target_model_from_cube_id(cube_id)
                except CubeIdentityError as exc:
                    raise BackendError(str(exc), status=400) from exc
                if path_target:
                    raise BackendError(
                        "metadata.target_model is required for target-folder cubes",
                        status=400,
                        details={"cube_id": cube_id, "path_target_model": path_target},
                    )
            remove_keys.add("target_model")
    if "supported_models" in payload:
        target_model = normalize_metadata_string(normalized.get("target_model"))
        if not target_model and cube_id:
            try:
                target_model = derive_target_model_from_cube_id(cube_id)
            except CubeIdentityError as exc:
                raise BackendError(str(exc), status=400) from exc
        supported_models = normalize_supported_models(
            payload.get("supported_models"),
            target_model=target_model,
        )
        if supported_models:
            normalized["supported_models"] = supported_models
        else:
            remove_keys.add("supported_models")
    if "lineage" in payload:
        lineage = normalize_lineage_payload(payload.get("lineage"))
        if lineage:
            normalized["lineage"] = lineage
        else:
            remove_keys.add("lineage")
    if "icon" in payload:
        icon_value = payload.get("icon")
        if icon_value is None:
            remove_keys.add("icon")
        else:
            try:
                icon = normalize_icon_metadata(icon_value)
            except CubeIconError as exc:
                raise BackendError(str(exc), status=400) from exc
            if icon:
                normalized["icon"] = icon
            else:
                remove_keys.add("icon")
    return normalized, remove_keys



