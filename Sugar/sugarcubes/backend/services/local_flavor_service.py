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
"""Disk-backed local flavor persistence for SugarCubes."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, MutableMapping, Sequence
from uuid import uuid4

from ...cube_model import (
    CubeIdentityError,
    dedupe_flavor_id,
    normalize_flavor_id,
    parse_canonical_cube_id,
)
from ..responses import BackendError
from .tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_LOCAL_FLAVOR_SCHEMA_VERSION = 1
_WHITESPACE_RE = re.compile(r"\s+")


class LocalFlavorService:
    """Own local flavor state under the managed local SugarCubes workspace."""

    def __init__(self, tracked_repo_service: TrackedRepoService) -> None:
        """Initialize the local flavor store with the tracked repo owner."""

        self.tracked_repo_service = tracked_repo_service

    def store_root(self) -> Path:
        """Return the canonical local flavor JSON directory."""

        return (
            self.tracked_repo_service.ensure_local_repo() / "flavors" / "by-cube"
        ).resolve()

    def local_flavor_root(self) -> Path:
        """Return the root passed to Sugar package catalog readers."""

        return (self.tracked_repo_service.ensure_local_repo() / "flavors").resolve()

    def path_for_cube_id(self, cube_id: str) -> Path:
        """Return the deterministic state path for one canonical cube id."""

        canonical_cube_id = self._canonical_cube_id(cube_id)
        digest = hashlib.sha256(canonical_cube_id.encode("utf-8")).hexdigest()
        return self.store_root() / f"{digest}.json"

    def read_cube_state(self, cube_id: str) -> dict[str, Any]:
        """Read one cube's local flavor state or return an empty state."""

        canonical_cube_id = self._canonical_cube_id(cube_id)
        path = self.path_for_cube_id(canonical_cube_id)
        if not path.exists():
            return self._empty_state(canonical_cube_id)
        try:
            with path.open("r", encoding="utf-8") as handle:
                payload = json.load(handle)
        except json.JSONDecodeError as exc:
            raise BackendError(
                "Local flavor state is not valid JSON",
                status=500,
                details={"cube_id": canonical_cube_id},
            ) from exc
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to read local flavor state for cube '%s'",
                canonical_cube_id,
                exc_info=exc,
            )
            raise BackendError("Failed to read local flavors", status=500) from exc
        return self._normalize_state(payload, canonical_cube_id)

    def write_cube_state(
        self, cube_id: str, state: Mapping[str, Any]
    ) -> dict[str, Any]:
        """Validate and atomically persist one cube's local flavor state."""

        canonical_cube_id = self._canonical_cube_id(cube_id)
        normalized = self._normalize_state(state, canonical_cube_id)
        normalized["updated_at"] = _utc_now()
        path = self.path_for_cube_id(canonical_cube_id)
        path.parent.mkdir(parents=True, exist_ok=True)
        temp_path = path.with_name(f"{path.name}.{uuid4().hex}.tmp")
        try:
            with temp_path.open("w", encoding="utf-8") as handle:
                json.dump(normalized, handle, indent=2, sort_keys=True)
                handle.write("\n")
            os.replace(temp_path, path)
        except (OSError, TypeError, ValueError) as exc:
            _logger.exception(
                "SugarCubes: failed to write local flavor state for cube '%s'",
                canonical_cube_id,
                exc_info=exc,
            )
            raise BackendError("Failed to write local flavors", status=500) from exc
        finally:
            try:
                if temp_path.exists():
                    temp_path.unlink()
            except OSError:
                _logger.warning(
                    "SugarCubes: failed to remove local flavor temp file",
                    exc_info=True,
                )
        return normalized

    def move_cube_state(
        self, source_cube_id: str, target_cube_id: str
    ) -> dict[str, Any]:
        """Move machine-local flavor state after a canonical cube identity change."""

        source_id = self._canonical_cube_id(source_cube_id)
        target_id = self._canonical_cube_id(target_cube_id)
        source_path = self.path_for_cube_id(source_id)
        target_path = self.path_for_cube_id(target_id)
        if not source_path.exists():
            return {
                "moved": False,
                "source_cube_id": source_id,
                "target_cube_id": target_id,
            }
        if target_path.exists():
            raise BackendError(
                "Local flavor state already exists for the target cube", status=409
            )
        state = self.read_cube_state(source_id)
        state["cube_id"] = target_id
        self.write_cube_state(target_id, state)
        try:
            source_path.unlink()
        except OSError as exc:
            try:
                target_path.unlink()
            except OSError:
                _logger.warning(
                    "SugarCubes: failed to roll back target flavor state",
                    exc_info=True,
                )
            _logger.exception(
                "SugarCubes: failed to retire local flavor state for '%s'",
                source_id,
            )
            raise BackendError("Failed to move local flavor state", status=500) from exc
        return {"moved": True, "source_cube_id": source_id, "target_cube_id": target_id}

    def save_local_flavor(
        self,
        *,
        cube_id: str,
        surface_signature: str,
        name: str,
        values: Mapping[str, Any],
        flavor_id: str | None = None,
        authored_flavors: Sequence[Mapping[str, Any]] = (),
    ) -> dict[str, Any]:
        """Save one local flavor entry and return the updated cube state."""

        state = self.read_cube_state(cube_id)
        signature = self._require_surface_signature(surface_signature)
        surface_state = self._surface_state(state, signature)
        resolved_name = _normalize_display_name(name) or "Local Flavor"
        requested_id = _normalize_id(flavor_id)
        if requested_id == "default":
            raise BackendError("Local flavor id 'default' is reserved", status=400)
        authored_keys = _build_flavor_keys(authored_flavors)
        _raise_if_authored_collision(
            flavor_id=requested_id or normalize_flavor_id(resolved_name),
            flavor_name=resolved_name,
            authored_keys=authored_keys,
        )
        existing_id = requested_id or None
        used_ids = {
            _normalize_id(entry.get("id"))
            for entry in surface_state["flavors"]
            if _normalize_id(entry.get("id"))
            and _normalize_id(entry.get("id")) != existing_id
        }
        used_ids.update(authored_keys["ids"])
        used_ids.add("default")
        resolved_id = existing_id or dedupe_flavor_id(
            normalize_flavor_id(resolved_name), used_ids
        )
        _raise_if_local_name_collision(
            resolved_id=resolved_id,
            flavor_name=resolved_name,
            flavors=surface_state["flavors"],
        )
        next_flavor = {
            "id": resolved_id,
            "name": resolved_name,
            "values": _json_clone(dict(values)),
            "updated_at": _utc_now(),
        }
        surface_state["flavors"] = [
            entry
            for entry in surface_state["flavors"]
            if _normalize_id(entry.get("id")) != resolved_id
        ]
        surface_state["flavors"].append(next_flavor)
        surface_state["selected_flavor_id"] = resolved_id
        state["surfaces"][signature] = surface_state
        return self.write_cube_state(cube_id, state)

    def delete_local_flavor(
        self,
        *,
        cube_id: str,
        surface_signature: str,
        flavor_id: str,
    ) -> dict[str, Any]:
        """Delete one local flavor entry and return the updated cube state."""

        state = self.read_cube_state(cube_id)
        signature = self._require_surface_signature(surface_signature)
        surface_state = self._surface_state(state, signature)
        target_id = _normalize_id(flavor_id)
        if not target_id:
            raise BackendError("'flavor_id' field is required", status=400)
        next_flavors = [
            entry
            for entry in surface_state["flavors"]
            if _normalize_id(entry.get("id")) != target_id
        ]
        surface_state["flavors"] = next_flavors
        if surface_state.get("selected_flavor_id") == target_id:
            surface_state["selected_flavor_id"] = ""
        state["surfaces"][signature] = surface_state
        return self.write_cube_state(cube_id, state)

    def set_selected_flavor(
        self,
        *,
        cube_id: str,
        surface_signature: str,
        flavor_id: str,
    ) -> dict[str, Any]:
        """Persist the selected local flavor id for one cube surface."""

        state = self.read_cube_state(cube_id)
        signature = self._require_surface_signature(surface_signature)
        surface_state = self._surface_state(state, signature)
        selected_id = _normalize_id(flavor_id)
        if selected_id and not any(
            _normalize_id(entry.get("id")) == selected_id
            for entry in surface_state["flavors"]
        ):
            raise BackendError("Local flavor not found", status=404)
        surface_state["selected_flavor_id"] = selected_id
        state["surfaces"][signature] = surface_state
        return self.write_cube_state(cube_id, state)

    def migrate_states(self, states: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
        """Migrate browser-provided local flavor states into disk storage."""

        migrated: list[dict[str, str]] = []
        for entry in states:
            cube_id = entry.get("cube_id") if isinstance(entry, Mapping) else ""
            state = entry.get("state") if isinstance(entry, Mapping) else None
            if not isinstance(cube_id, str) or not isinstance(state, Mapping):
                raise BackendError("Migration state entries are invalid", status=400)
            normalized = self._normalize_state(state, self._canonical_cube_id(cube_id))
            self.write_cube_state(cube_id, normalized)
            migrated.append({"cube_id": self._canonical_cube_id(cube_id)})
        return {"migrated": migrated, "count": len(migrated)}

    def reconcile_with_authored_flavors(
        self,
        *,
        cube_id: str,
        surface_signature: str,
        authored_flavors: Sequence[Mapping[str, Any]],
        rename_map: Mapping[str, str] | None = None,
    ) -> dict[str, Any]:
        """Rename local flavors that collide with authored flavor names or ids."""

        state = self.read_cube_state(cube_id)
        signature = self._require_surface_signature(surface_signature)
        surface_state = self._surface_state(state, signature)
        authored_keys = _build_flavor_keys(authored_flavors)
        rename_map = rename_map or {}
        used_ids = set(authored_keys["ids"])
        used_names = set(authored_keys["names"])
        renamed: list[dict[str, str]] = []
        selected_id = surface_state.get("selected_flavor_id") or ""
        next_flavors: list[dict[str, Any]] = []

        for flavor in surface_state["flavors"]:
            current_id = _normalize_id(flavor.get("id"))
            current_name = _normalize_display_name(flavor.get("name")) or current_id
            collides = (
                current_id in authored_keys["ids"]
                or _name_key(current_name) in authored_keys["names"]
            )
            if not collides:
                next_flavor = dict(flavor)
                used_ids.add(current_id)
                used_names.add(_name_key(current_name))
                next_flavors.append(next_flavor)
                continue

            requested_name = _normalize_display_name(rename_map.get(current_id, ""))
            next_name = self._resolve_reconciled_name(
                current_name=current_name,
                requested_name=requested_name,
                used_names=used_names,
            )
            next_id = dedupe_flavor_id(normalize_flavor_id(next_name), used_ids)
            next_flavor = {
                **dict(flavor),
                "id": next_id,
                "name": next_name,
                "updated_at": _utc_now(),
            }
            if selected_id == current_id:
                selected_id = next_id
            used_names.add(_name_key(next_name))
            next_flavors.append(next_flavor)
            renamed.append(
                {
                    "old_id": current_id,
                    "old_name": current_name,
                    "new_id": next_id,
                    "new_name": next_name,
                }
            )

        surface_state["flavors"] = next_flavors
        surface_state["selected_flavor_id"] = selected_id
        state["surfaces"][signature] = surface_state
        return {
            "state": self.write_cube_state(cube_id, state),
            "renamed": renamed,
            "conflict_count": len(renamed),
        }

    def find_authored_local_collisions(
        self,
        *,
        cube_id: str,
        surface_signature: str,
        authored_flavors: Sequence[Mapping[str, Any]],
    ) -> list[dict[str, str]]:
        """Return local flavors that collide with authored names or ids."""

        state = self.read_cube_state(cube_id)
        signature = self._require_surface_signature(surface_signature)
        surface_state = self._surface_state(state, signature)
        authored_keys = _build_flavor_keys(authored_flavors)
        collisions: list[dict[str, str]] = []
        for flavor in surface_state["flavors"]:
            flavor_id = _normalize_id(flavor.get("id"))
            flavor_name = _normalize_display_name(flavor.get("name")) or flavor_id
            reasons: list[str] = []
            if flavor_id in authored_keys["ids"]:
                reasons.append("id")
            if _name_key(flavor_name) in authored_keys["names"]:
                reasons.append("name")
            if reasons:
                collisions.append(
                    {
                        "id": flavor_id,
                        "name": flavor_name,
                        "reason": ",".join(reasons),
                    }
                )
        return collisions

    def _resolve_reconciled_name(
        self,
        *,
        current_name: str,
        requested_name: str,
        used_names: set[str],
    ) -> str:
        """Return a non-colliding local flavor display name."""

        if requested_name and _name_key(requested_name) not in used_names:
            return requested_name
        base = f"{current_name}_local"
        if _name_key(base) not in used_names:
            return base
        suffix = 2
        while True:
            candidate = f"{base}_{suffix}"
            if _name_key(candidate) not in used_names:
                return candidate
            suffix += 1

    def _canonical_cube_id(self, cube_id: str) -> str:
        """Normalize one cube id or raise a backend validation error."""

        try:
            return parse_canonical_cube_id(cube_id).to_string()
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc

    def _empty_state(self, cube_id: str) -> dict[str, Any]:
        """Build an empty versioned state payload for one cube."""

        return {
            "schema_version": _LOCAL_FLAVOR_SCHEMA_VERSION,
            "cube_id": cube_id,
            "surfaces": {},
            "updated_at": "",
        }

    def _normalize_state(self, payload: Any, expected_cube_id: str) -> dict[str, Any]:
        """Validate local flavor state and normalize it into schema version 1."""

        if not isinstance(payload, Mapping):
            raise BackendError("Local flavor state must be an object", status=400)
        schema_version = payload.get("schema_version", payload.get("schema"))
        if schema_version not in (_LOCAL_FLAVOR_SCHEMA_VERSION, None):
            raise BackendError("Unsupported local flavor schema version", status=400)
        cube_id = self._canonical_cube_id(
            str(payload.get("cube_id") or expected_cube_id)
        )
        if cube_id != expected_cube_id:
            raise BackendError("Local flavor state cube id mismatch", status=400)
        surfaces_payload = payload.get("surfaces")
        if surfaces_payload is None:
            surfaces_payload = {}
        if not isinstance(surfaces_payload, Mapping):
            raise BackendError("Local flavor surfaces must be an object", status=400)
        surfaces: dict[str, dict[str, Any]] = {}
        for signature, surface_state in surfaces_payload.items():
            signature_key = self._require_surface_signature(signature)
            surfaces[signature_key] = self._normalize_surface_state(surface_state)
        return {
            **{
                key: _json_clone(value)
                for key, value in payload.items()
                if key not in {"schema", "schema_version", "cube_id", "surfaces"}
            },
            "schema_version": _LOCAL_FLAVOR_SCHEMA_VERSION,
            "cube_id": cube_id,
            "surfaces": surfaces,
            "updated_at": str(payload.get("updated_at") or ""),
        }

    def _normalize_surface_state(self, payload: Any) -> dict[str, Any]:
        """Normalize one surface's local flavor entries."""

        if not isinstance(payload, Mapping):
            raise BackendError(
                "Local flavor surface state must be an object", status=400
            )
        flavors_payload = payload.get("flavors")
        if flavors_payload is None:
            flavors_payload = []
        if not isinstance(flavors_payload, list):
            raise BackendError("Local flavor entries must be an array", status=400)
        flavors = [self._normalize_flavor_entry(entry) for entry in flavors_payload]
        selected_id = _normalize_id(payload.get("selected_flavor_id"))
        return {
            **{
                key: _json_clone(value)
                for key, value in payload.items()
                if key not in {"selected_flavor_id", "flavors"}
            },
            "selected_flavor_id": selected_id,
            "flavors": flavors,
        }

    def _normalize_flavor_entry(self, payload: Any) -> dict[str, Any]:
        """Normalize one local flavor entry."""

        if not isinstance(payload, Mapping):
            raise BackendError("Local flavor entry must be an object", status=400)
        flavor_id = _normalize_id(payload.get("id"))
        if not flavor_id:
            raise BackendError("Local flavor id is required", status=400)
        if flavor_id == "default":
            raise BackendError("Local flavor id 'default' is reserved", status=400)
        name = _normalize_display_name(payload.get("name")) or flavor_id
        values = payload.get("values")
        if values is None:
            values = {}
        if not isinstance(values, Mapping):
            raise BackendError("Local flavor values must be an object", status=400)
        return {
            **{
                key: _json_clone(value)
                for key, value in payload.items()
                if key not in {"id", "name", "values", "updated_at"}
            },
            "id": flavor_id,
            "name": name,
            "values": _json_clone(dict(values)),
            "updated_at": str(payload.get("updated_at") or ""),
        }

    def _surface_state(
        self, state: MutableMapping[str, Any], signature: str
    ) -> dict[str, Any]:
        """Return an existing or empty normalized surface state."""

        surfaces = state.setdefault("surfaces", {})
        if not isinstance(surfaces, dict):
            raise BackendError("Local flavor surfaces must be an object", status=400)
        surface_state = surfaces.get(signature)
        if surface_state is None:
            return {"selected_flavor_id": "", "flavors": []}
        return self._normalize_surface_state(surface_state)

    def _require_surface_signature(self, surface_signature: Any) -> str:
        """Return a non-empty surface signature or raise a backend error."""

        signature = str(surface_signature or "").strip()
        if not signature:
            raise BackendError("'surface_signature' field is required", status=400)
        return signature


def _utc_now() -> str:
    """Return the current UTC timestamp for persisted state."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")


def _json_clone(value: Any) -> Any:
    """Return a JSON-compatible deep copy or raise a backend error."""

    try:
        return json.loads(json.dumps(value))
    except (TypeError, ValueError) as exc:
        raise BackendError(
            "Local flavor payload must be JSON serializable", status=400
        ) from exc


def _normalize_id(value: Any) -> str:
    """Normalize a persisted local flavor id."""

    raw_value = str(value or "").strip()
    return normalize_flavor_id(raw_value) if raw_value else ""


def _normalize_display_name(value: Any) -> str:
    """Normalize flavor display text for storage and comparisons."""

    if not isinstance(value, str):
        return ""
    return _WHITESPACE_RE.sub(" ", value.strip())


def _name_key(value: Any) -> str:
    """Return the collision key for a flavor display name."""

    return _normalize_display_name(value).casefold()


def _build_flavor_keys(flavors: Sequence[Mapping[str, Any]]) -> dict[str, set[str]]:
    """Build normalized authored/local flavor collision keys."""

    ids: set[str] = set()
    names: set[str] = set()
    for flavor in flavors:
        flavor_id = (
            _normalize_id(flavor.get("id")) if isinstance(flavor, Mapping) else ""
        )
        if flavor_id:
            ids.add(flavor_id)
        name = _name_key(flavor.get("name")) if isinstance(flavor, Mapping) else ""
        if name:
            names.add(name)
    return {"ids": ids, "names": names}


def _raise_if_authored_collision(
    *,
    flavor_id: str,
    flavor_name: str,
    authored_keys: Mapping[str, set[str]],
) -> None:
    """Reject a local flavor that conflicts with authored flavor keys."""

    if flavor_id and flavor_id in authored_keys["ids"]:
        raise BackendError(
            "Local flavor id collides with an authored flavor",
            status=409,
            details={"flavor_id": flavor_id},
        )
    name = _name_key(flavor_name)
    if name and name in authored_keys["names"]:
        raise BackendError(
            "Local flavor name collides with an authored flavor",
            status=409,
            details={"flavor_name": flavor_name},
        )


def _raise_if_local_name_collision(
    *,
    resolved_id: str,
    flavor_name: str,
    flavors: Sequence[Mapping[str, Any]],
) -> None:
    """Reject ambiguous local flavor display names."""

    name = _name_key(flavor_name)
    for flavor in flavors:
        if _normalize_id(flavor.get("id")) == resolved_id:
            continue
        if _name_key(flavor.get("name")) == name:
            raise BackendError(
                "Local flavor name already exists for this surface",
                status=409,
                details={"flavor_name": flavor_name},
            )
