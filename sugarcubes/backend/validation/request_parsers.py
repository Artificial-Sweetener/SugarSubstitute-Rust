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
"""Request parsing and normalization helpers for SugarCubes routes."""

from __future__ import annotations

import json
from typing import Any, Mapping, Optional, Sequence

from aiohttp.client_exceptions import ContentTypeError

from ...cube_model import CubeIdentityError, parse_canonical_cube_id
from ..responses import BackendError
from ..services.cube_metadata import (
    normalize_lineage_payload,
    normalize_metadata_string,
    normalize_metadata_update,
)

_JSON_BODY_ERRORS = (
    ContentTypeError,
    json.JSONDecodeError,
    TypeError,
    ValueError,
)


def _request_body_is_empty(request: Any) -> bool:
    """Return whether the request metadata proves the body is empty."""

    content_length = getattr(request, "content_length", None)
    if content_length == 0:
        return True
    can_read_body = getattr(request, "can_read_body", None)
    return can_read_body is False


async def _read_json_mapping(
    request: Any,
    *,
    allow_empty: bool,
) -> Optional[Mapping[str, Any]]:
    """Read a JSON mapping body and centralize invalid-body handling.

    Args:
        request: aiohttp-like request object that exposes `json()`.
        allow_empty: Return `None` when the route explicitly permits an empty body.

    Returns:
        The parsed JSON mapping or `None` when empty bodies are allowed.

    Raises:
        BackendError: The JSON is malformed or the payload is not an object.
    """

    try:
        body = await request.json()
    except _JSON_BODY_ERRORS as exc:
        if allow_empty and _request_body_is_empty(request):
            return None
        raise BackendError("Invalid JSON body", status=400) from exc
    if body is None and allow_empty:
        return None
    if not isinstance(body, Mapping):
        raise BackendError("Request body must be a JSON object", status=400)
    return body


async def parse_json_body(request: Any) -> Mapping[str, Any]:
    """Read a JSON body and require a top-level object payload."""

    body = await _read_json_mapping(request, allow_empty=False)
    assert body is not None
    return body


async def parse_optional_json_body(request: Any) -> Optional[Mapping[str, Any]]:
    """Read an optional JSON object body for routes that accept query-only deletes.

    The delete route accepts either query parameters or a JSON object body. Empty
    bodies therefore map to `None`, while malformed JSON still remains a 400.
    """

    return await _read_json_mapping(request, allow_empty=True)


def extract_drop_origin(value: Any) -> Optional[list[float]]:
    """Normalize the drop origin into a two-element list when possible."""

    if value is None:
        return None
    if isinstance(value, Mapping):
        return [value.get("x", 0.0), value.get("y", 0.0)]
    if isinstance(value, (list, tuple)) and len(value) == 2:
        return [value[0], value[1]]
    return None


def normalize_graph_payload(payload: Any) -> dict[str, Any]:
    """Coerce the incoming graph payload into a JSON object."""

    if isinstance(payload, str):
        try:
            payload = json.loads(payload)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive
            raise BackendError(
                "Request 'graph' must be valid JSON", status=400
            ) from exc
    if not isinstance(payload, dict):
        raise BackendError("Request 'graph' must be a JSON object", status=400)
    return payload


def normalize_workflow_payload(payload: Any) -> dict[str, Any]:
    """Coerce the incoming workflow payload into a JSON object."""

    if payload is None:
        raise BackendError("Request 'workflow' is required", status=400)
    if isinstance(payload, str):
        try:
            payload = json.loads(payload)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive
            raise BackendError(
                "Request 'workflow' must be valid JSON", status=400
            ) from exc
    if not isinstance(payload, Mapping):
        raise BackendError("Request 'workflow' must be a JSON object", status=400)
    return dict(payload)


def get_bool(payload: Mapping[str, Any], key: str, default: bool = False) -> bool:
    """Coerce a mapping value into a boolean flag."""

    value = payload.get(key, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in {"1", "true", "yes", "on"}:
            return True
        if normalized in {"0", "false", "no", "off"}:
            return False
        return default
    if isinstance(value, (int, float)):
        return bool(value)
    return default


def coerce_int(value: Any, default: Optional[int] = None) -> Optional[int]:
    """Coerce an arbitrary value to an integer when possible."""

    if value is None or isinstance(value, bool):
        return default
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped:
            return default
        try:
            return int(stripped)
        except ValueError:
            return default
    try:
        return int(str(value))
    except (TypeError, ValueError):
        return default


def normalize_actor(value: Any) -> Optional[dict[str, str]]:
    """Normalize the exporting actor payload."""

    if not isinstance(value, Mapping):
        return None
    author = normalize_metadata_string(value.get("author"))
    author_url = normalize_metadata_string(value.get("author_url"))
    if not author:
        return None
    payload = {"author": author}
    if author_url:
        payload["author_url"] = author_url
    return payload


def parse_save_many_cube_entries(value: Any) -> dict[str, dict[str, Any]]:
    """Normalize save-many cube entries while preserving current validation behavior."""

    if not isinstance(value, Sequence):
        raise BackendError("'cubes' field must be a list", status=400)

    entries: dict[str, dict[str, Any]] = {}
    for entry in value:
        if not isinstance(entry, Mapping):
            raise BackendError("Cube entry must be an object", status=400)
        cube_id = normalize_metadata_string(entry.get("cube_id"))
        if not cube_id:
            raise BackendError("Cube entry missing cube_id", status=400)
        try:
            parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        if cube_id in entries:
            raise BackendError(f"Duplicate cube_id '{cube_id}' in request", status=400)
        previous_cube_id = normalize_metadata_string(entry.get("previous_cube_id"))
        if previous_cube_id:
            try:
                parse_canonical_cube_id(previous_cube_id)
            except CubeIdentityError as exc:
                raise BackendError(str(exc), status=400) from exc
        stale_save_mode = normalize_metadata_string(entry.get("stale_save_mode"))
        if stale_save_mode and stale_save_mode != "latest":
            raise BackendError(
                "'stale_save_mode' must be 'latest' when provided", status=400
            )
        metadata_payload = entry.get("metadata")
        metadata_updates: dict[str, Any] = {}
        if metadata_payload is not None:
            if not isinstance(metadata_payload, Mapping):
                raise BackendError("Cube entry metadata must be an object", status=400)
            metadata_updates, _metadata_removals = normalize_metadata_update(
                metadata_payload,
                cube_id=cube_id,
            )
        entries[cube_id] = {
            "forked": get_bool(entry, "forked", False),
            "lineage": normalize_lineage_payload(entry.get("lineage")),
            "metadata": metadata_updates,
            "previous_cube_id": previous_cube_id,
            "description_set": "description" in entry,
            "description": normalize_metadata_string(entry.get("description")),
            "source_revision_ref": normalize_metadata_string(
                entry.get("source_revision_ref")
            ),
            "source_version": normalize_metadata_string(entry.get("source_version")),
            "source_definition_key": normalize_metadata_string(
                entry.get("source_definition_key")
            ),
            "stale_save_mode": stale_save_mode,
        }
    return entries
