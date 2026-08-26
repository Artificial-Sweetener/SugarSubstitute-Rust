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
"""Persist exact wild Cube definitions in a content-addressed read-only Stable."""

from __future__ import annotations

import json
import os
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping

from ..workflow import EmbeddedCubeDefinition
from .models import CatalogCubeArtifact

_HASH_PATTERN = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class StableWrite:
    """Report whether one exact Stable object was newly created."""

    created: bool
    artifact: CatalogCubeArtifact


class StableCubeRepository:
    """Own content-addressed Stable objects and their claimed-identity index."""

    def __init__(self, root: Path) -> None:
        """Bind one dedicated Stable root without touching it eagerly."""

        self._root = root.resolve()
        self._objects = self._root / "objects"
        self._index = self._root / "index.json"

    def save(self, definition: EmbeddedCubeDefinition) -> StableWrite:
        """Store exact embedded bytes idempotently under their semantic hash."""

        content_hash = _require_hash(definition.semantic_hash)
        self._objects.mkdir(parents=True, exist_ok=True)
        object_path = self._objects / f"{content_hash}.json"
        created = not object_path.exists()
        if created:
            _write_json_atomic(object_path, definition.payload)
        entries = self._read_index()
        key = _index_key(definition.cube_id, definition.cube_version, content_hash)
        if key not in entries:
            entries[key] = {
                "cube_id": definition.cube_id,
                "cube_version": definition.cube_version,
                "semantic_hash": content_hash,
                "object": f"objects/{content_hash}.json",
                "provenance": definition.provenance,
            }
            self._root.mkdir(parents=True, exist_ok=True)
            _write_json_atomic(self._index, {"schema_version": 1, "entries": entries})
        return StableWrite(created=created, artifact=_artifact(definition))

    def list_artifacts(self) -> tuple[CatalogCubeArtifact, ...]:
        """Return deterministic read-only candidates from the Stable index."""

        artifacts: list[CatalogCubeArtifact] = []
        for entry in self._read_index().values():
            cube_id = _read_string(entry.get("cube_id"))
            cube_version = _read_string(entry.get("cube_version"))
            semantic_hash = _read_string(entry.get("semantic_hash"))
            if (
                not cube_id
                or not cube_version
                or not _HASH_PATTERN.fullmatch(semantic_hash)
            ):
                continue
            artifacts.append(
                CatalogCubeArtifact(
                    cube_id=cube_id,
                    cube_version=cube_version,
                    semantic_hash=semantic_hash,
                    library_class="stable",
                    access="read_only",
                    source_ref=f"stable:{semantic_hash}",
                )
            )
        return tuple(sorted(artifacts, key=lambda item: item.source_ref))

    def _read_index(self) -> dict[str, dict[str, object]]:
        """Read valid index entries while treating a missing Stable as empty."""

        if not self._index.exists():
            return {}
        raw = json.loads(self._index.read_text(encoding="utf-8"))
        if not isinstance(raw, Mapping) or raw.get("schema_version") != 1:
            raise ValueError("Wild Cube Stable index is invalid")
        entries = raw.get("entries")
        if not isinstance(entries, Mapping):
            raise ValueError("Wild Cube Stable index entries are invalid")
        result: dict[str, dict[str, object]] = {}
        for key, value in entries.items():
            if isinstance(key, str) and isinstance(value, Mapping):
                result[key] = dict(value)
        return result


def _artifact(definition: EmbeddedCubeDefinition) -> CatalogCubeArtifact:
    """Project one embedded definition into its Stable catalog record."""

    return CatalogCubeArtifact(
        cube_id=definition.cube_id,
        cube_version=definition.cube_version,
        semantic_hash=definition.semantic_hash,
        library_class="stable",
        access="read_only",
        source_ref=f"stable:{definition.semantic_hash}",
    )


def _write_json_atomic(path: Path, payload: object) -> None:
    """Replace one Stable JSON file atomically after a complete local write."""

    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
            json.dump(payload, stream, ensure_ascii=False, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_path, path)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise


def _require_hash(value: str) -> str:
    """Reject any content identity that cannot be a safe object filename."""

    if not _HASH_PATTERN.fullmatch(value):
        raise ValueError("Stable Cube semantic hash is invalid")
    return value


def _index_key(cube_id: str, cube_version: str, semantic_hash: str) -> str:
    """Build an unambiguous index key without using identity text as a path."""

    return json.dumps([cube_id, cube_version, semantic_hash], separators=(",", ":"))


def _read_string(value: object) -> str:
    """Read optional normalized index text."""

    return value.strip() if isinstance(value, str) else ""
