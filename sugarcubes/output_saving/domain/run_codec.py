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
"""Encode immutable saving snapshots for the server-owned ticket journal."""

from __future__ import annotations

from collections.abc import Mapping
from datetime import datetime

from .preference_codec import decode_preferences, encode_preferences
from .run_plan import SavingRun, SavingSource, SourceKind


def encode_run(run: SavingRun) -> dict[str, object]:
    """Persist one policy and metadata snapshot separately from its output tickets."""
    return {
        "schema_version": 1,
        "run_id": run.run_id,
        "root": run.root,
        "workflow": run.workflow,
        "started_at": run.started_at.isoformat(),
        "preferences": encode_preferences(run.preferences),
        "metadata": dict(run.metadata),
        "metadata_enabled": run.metadata_enabled,
        "generation_seed": run.generation_seed,
    }


def decode_run(value: object) -> SavingRun:
    """Validate journal bytes before granting a runtime sink any filesystem authority."""
    payload = _record(value)
    if (
        type(payload.get("schema_version")) is not int
        or payload.get("schema_version") != 1
    ):
        raise ValueError("Unsupported saving run snapshot version.")
    metadata = _record(payload.get("metadata"))
    metadata_enabled = payload.get("metadata_enabled")
    if not isinstance(metadata_enabled, bool):
        raise ValueError("Saving run metadata flag must be a boolean.")
    generation_seed = payload.get("generation_seed")
    if generation_seed is not None and not isinstance(generation_seed, str):
        raise ValueError("Saving run seed must be text or null.")
    return SavingRun(
        run_id=_text(payload.get("run_id"), allow_empty=False),
        root=_text(payload.get("root"), allow_empty=False),
        workflow=_text(payload.get("workflow")),
        started_at=datetime.fromisoformat(_text(payload.get("started_at"))),
        preferences=decode_preferences(payload.get("preferences")),
        metadata=tuple((key, _text(item)) for key, item in metadata.items()),
        metadata_enabled=metadata_enabled,
        generation_seed=generation_seed,
    )


def encode_source(source: SavingSource) -> dict[str, object]:
    """Store source policy without repeating potentially large workflow metadata."""
    return {
        "node_id": source.node_id,
        "label": source.label,
        "kind": source.kind.value,
        "cube_id": source.cube_id,
        "cube_number": source.cube_number,
        "cube_alias": source.cube_alias,
        "seed": source.seed,
        "enabled": source.enabled,
        "persist": source.persist,
    }


def decode_source(value: object) -> SavingSource:
    """Narrow persisted source identities and mute state before they reach runtime policy."""
    payload = _record(value)
    cube_number = payload.get("cube_number")
    if cube_number is not None and (
        not isinstance(cube_number, int)
        or isinstance(cube_number, bool)
        or cube_number < 1
    ):
        raise ValueError("Saving source cube number must be positive or null.")
    enabled, persist = payload.get("enabled"), payload.get("persist")
    if not isinstance(enabled, bool) or not isinstance(persist, bool):
        raise ValueError("Saving source flags must be booleans.")
    return SavingSource(
        node_id=_text(payload.get("node_id"), allow_empty=False),
        label=_text(payload.get("label")),
        kind=SourceKind(_text(payload.get("kind"))),
        cube_id=_text(payload.get("cube_id")),
        cube_number=cube_number,
        cube_alias=_text(payload.get("cube_alias")),
        seed=_text(payload.get("seed")),
        enabled=enabled,
        persist=persist,
    )


def _record(value: object) -> Mapping[str, object]:
    """Accept only named persisted fields rather than arbitrary Python objects."""
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise ValueError("Saving snapshot must be an object with string keys.")
    return {key: item for key, item in value.items() if isinstance(key, str)}


def _text(value: object, *, allow_empty: bool = True) -> str:
    """Reject malformed snapshot text without normalizing meaningful user labels."""
    if not isinstance(value, str) or (not allow_empty and not value):
        raise ValueError("Saving snapshot text is invalid.")
    return value
