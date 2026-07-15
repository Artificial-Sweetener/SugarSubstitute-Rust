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
"""Route-based cube identity helpers and model-family metadata normalization."""

from __future__ import annotations

from pathlib import PurePosixPath
import re
from typing import Any

from .cube_identity import (
    CubeIdentityError,
    build_canonical_cube_id,
    parse_canonical_cube_id,
    suggest_canonical_cube_path,
)

_WINDOWS_UNSAFE_PATH_SEGMENT_RE = re.compile(r'[<>:"|?*\x00-\x1f]')
_WHITESPACE_RE = re.compile(r"\s+")
_MODEL_LABEL_ALIASES = {
    "sdxl 1.0": "SDXL",
}
ANY_TARGET_MODEL = "Any"


def normalize_target_model(value: Any) -> str:
    """Return a path-safe target model label or an empty string."""

    if not isinstance(value, str):
        return ""
    normalized = _normalize_model_label(value)
    if not normalized:
        return ""
    _validate_target_model_segment(normalized)
    return normalized


def normalize_supported_models(value: object, *, target_model: str = "") -> list[str]:
    """Return ordered supported model labels, including the target model when required."""

    models: list[str] = []
    raw_items: list[object]
    if isinstance(value, str):
        raw_items = list(value.split(","))
    elif isinstance(value, list):
        raw_items = list(value)
    else:
        raw_items = []

    for item in raw_items:
        if not isinstance(item, str):
            continue
        normalized = _normalize_model_label(item)
        if normalized:
            models.append(normalized)

    normalized_target = normalize_target_model(target_model)
    if normalized_target and normalized_target != ANY_TARGET_MODEL:
        models.insert(0, normalized_target)
    return _dedupe_preserving_order(models)


def normalize_cube_route(value: Any) -> str:
    """Return a path-safe cube route without a `.cube` suffix."""

    if not isinstance(value, str):
        return ""
    normalized = _WHITESPACE_RE.sub(" ", value.strip().replace("\\", "/"))
    if normalized.lower().endswith(".cube"):
        normalized = normalized[:-5]
    if not normalized:
        return ""
    segments = normalized.split("/")
    cleaned_segments: list[str] = []
    for segment in segments:
        cleaned = _WHITESPACE_RE.sub(" ", segment.strip())
        _validate_route_segment(cleaned)
        cleaned_segments.append(cleaned)
    return "/".join(cleaned_segments)


def derive_route_from_cube_id(cube_id: str) -> str:
    """Return the source-relative cube route used as the default alias."""

    parsed = parse_canonical_cube_id(cube_id)
    path = PurePosixPath(parsed.path)
    stem_path = path.with_suffix("").as_posix()
    return normalize_cube_route(stem_path)


def derive_target_model_from_route(route: str) -> str:
    """Return the target model segment implied by a cube route."""

    normalized = normalize_cube_route(route)
    if "/" not in normalized:
        return ""
    return normalize_target_model(normalized.split("/", 1)[0])


def derive_target_model_from_cube_id(cube_id: str) -> str:
    """Return the target model segment implied by a canonical cube id route."""

    return derive_target_model_from_route(derive_route_from_cube_id(cube_id))


def derive_filename_from_route(route: str) -> str:
    """Return the cube filename implied by a route."""

    normalized = normalize_cube_route(route)
    if not normalized:
        raise CubeIdentityError("Cube route is required")
    return suggest_canonical_cube_path(normalized.split("/")[-1])


def derive_cube_id_from_route(*, source_cube_id: str, route: str) -> str:
    """Build a canonical cube id with the same source and a route-derived path."""

    parsed = parse_canonical_cube_id(source_cube_id)
    normalized_route = normalize_cube_route(route)
    if not normalized_route:
        raise CubeIdentityError("Cube route is required")
    path_segments = normalized_route.split("/")
    path_segments[-1] = derive_filename_from_route(path_segments[-1])
    return build_canonical_cube_id(
        source_kind=parsed.source_kind,
        owner=parsed.owner,
        repo=parsed.repo,
        namespace=parsed.namespace,
        path="/".join(path_segments),
    )


def validate_cube_route_identity(cube_id: str, default_alias: str) -> None:
    """Validate that a persisted default alias matches its canonical cube id route."""

    expected = derive_route_from_cube_id(cube_id)
    actual = normalize_cube_route(default_alias)
    if actual != expected:
        raise CubeIdentityError(
            f"Cube default_alias must match cube route '{expected}'"
        )


def derive_target_model_cube_id(
    *,
    source_cube_id: str,
    target_model: str,
    default_alias: str,
) -> str:
    """Build a canonical cube id under the target-model folder."""

    parsed = parse_canonical_cube_id(source_cube_id)
    normalized_target = normalize_target_model(target_model)
    if not normalized_target:
        raise CubeIdentityError("Cube target model is required")
    name_route = normalize_cube_route(default_alias)
    name = name_route.split("/")[-1] if name_route else "cube"
    target_path = f"{normalized_target}/{suggest_canonical_cube_path(name)}"
    return build_canonical_cube_id(
        source_kind=parsed.source_kind,
        owner=parsed.owner,
        repo=parsed.repo,
        namespace=parsed.namespace,
        path=target_path,
    )


def _validate_route_segment(value: str) -> None:
    """Reject cube route segments that cannot be used in one repo path segment."""

    if not value:
        raise CubeIdentityError("Cube route must not contain empty segments")
    if value in {".", ".."}:
        raise CubeIdentityError("Cube route segment is invalid")
    if value[-1] in {" ", "."}:
        raise CubeIdentityError("Cube route segment must not end with a space or dot")
    if _WINDOWS_UNSAFE_PATH_SEGMENT_RE.search(value):
        raise CubeIdentityError("Cube route segment contains invalid characters")


def _validate_target_model_segment(value: str) -> None:
    """Reject target model labels that cannot be used as one repo path segment."""

    if value in {".", ".."}:
        raise CubeIdentityError("Cube target model is invalid")
    if "/" in value or "\\" in value:
        raise CubeIdentityError("Cube target model must be one path segment")
    if value[-1] in {" ", "."}:
        raise CubeIdentityError("Cube target model must not end with a space or dot")
    if _WINDOWS_UNSAFE_PATH_SEGMENT_RE.search(value):
        raise CubeIdentityError("Cube target model contains invalid characters")


def _normalize_model_label(value: str) -> str:
    """Return a canonical model-family label for user-facing metadata."""

    normalized = _WHITESPACE_RE.sub(" ", value.strip())
    return _MODEL_LABEL_ALIASES.get(normalized.casefold(), normalized)


def _dedupe_preserving_order(values: list[str]) -> list[str]:
    """Return values without duplicates while keeping first occurrence order."""

    seen: set[str] = set()
    deduped: list[str] = []
    for value in values:
        key = value.casefold()
        if key in seen:
            continue
        seen.add(key)
        deduped.append(value)
    return deduped
