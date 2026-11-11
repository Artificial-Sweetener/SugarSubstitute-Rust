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
"""Disk-backed cache for reconstructed historical cube version artifacts."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

_logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class CubeVersionArtifactCacheKey:
    """Describe the immutable source facts for one historical artifact."""

    cube_id: str
    version: str
    source_kind: str
    repo_identity: str
    repo_relative_path: str
    revision_ref: str


@dataclass(frozen=True)
class CubeVersionSelectionCacheKey:
    """Describe the source state used to select a version's newest revision."""

    cube_id: str
    version: str
    source_kind: str
    repo_identity: str
    repo_relative_path: str
    source_revision: str


class CubeVersionArtifactCache:
    """Persist reconstructed historical cube artifacts behind a bounded LRU."""

    _SCHEMA_VERSION = 1

    def __init__(
        self,
        root: Path,
        *,
        max_entries: int = 128,
        max_total_bytes: int = 256 * 1024 * 1024,
    ) -> None:
        """Initialize the cache rooted in SugarCubes runtime storage."""

        self.root = root.resolve()
        self.artifact_root = self.root / "artifacts"
        self.selection_root = self.root / "selections"
        self.max_entries = max_entries
        self.max_total_bytes = max_total_bytes

    def read_artifact(self, cache_key: str) -> dict[str, Any] | None:
        """Return one cached artifact and refresh its LRU timestamp."""

        path = self._artifact_path(cache_key)
        envelope = self._read_json(path)
        if envelope is None:
            return None
        artifact = envelope.get("artifact")
        if not isinstance(artifact, dict):
            self._delete_quietly(path)
            return None
        envelope["lastAccessedAt"] = time.time()
        envelope["byteSize"] = self._json_size(envelope)
        self._write_json_atomic(path, envelope)
        return dict(artifact)

    def write_artifact(self, cache_key: str, artifact: Mapping[str, Any]) -> None:
        """Persist one reconstructed artifact and prune stale entries."""

        envelope: dict[str, Any] = {
            "schemaVersion": self._SCHEMA_VERSION,
            "lastAccessedAt": time.time(),
            "byteSize": 0,
            "artifact": dict(artifact),
        }
        envelope["byteSize"] = self._json_size(envelope)
        self._write_json_atomic(self._artifact_path(cache_key), envelope)
        self.prune()

    def read_selection(self, cache_key: str) -> dict[str, Any] | None:
        """Return cached version selection metadata."""

        envelope = self._read_json(self._selection_path(cache_key))
        if envelope is None:
            return None
        selection = envelope.get("selection")
        return dict(selection) if isinstance(selection, dict) else None

    def write_selection(
        self,
        cache_key: str,
        *,
        revision_ref: str,
        content_hash: str,
        artifact_cache_key: str,
    ) -> None:
        """Persist the newest revision selected for one cube version."""

        envelope = {
            "schemaVersion": self._SCHEMA_VERSION,
            "selection": {
                "revisionRef": revision_ref,
                "contentHash": content_hash,
                "artifactCacheKey": artifact_cache_key,
                "timestamp": time.time(),
            },
        }
        self._write_json_atomic(self._selection_path(cache_key), envelope)

    def artifact_key(self, key: CubeVersionArtifactCacheKey) -> str:
        """Return a stable hashed artifact cache key."""

        return self._hash_mapping(
            {
                "cubeId": key.cube_id,
                "version": key.version,
                "sourceKind": key.source_kind,
                "repoIdentity": key.repo_identity,
                "repoRelativePath": key.repo_relative_path,
                "revisionRef": key.revision_ref,
            }
        )

    def selection_key(self, key: CubeVersionSelectionCacheKey) -> str:
        """Return a stable hashed version-selection cache key."""

        return self._hash_mapping(
            {
                "cubeId": key.cube_id,
                "version": key.version,
                "sourceKind": key.source_kind,
                "repoIdentity": key.repo_identity,
                "repoRelativePath": key.repo_relative_path,
                "sourceRevision": key.source_revision,
            }
        )

    def prune(self) -> None:
        """Best-effort prune by entry count and total byte size."""

        entries: list[tuple[Path, float, int]] = []
        for path in self.artifact_root.glob("*.json"):
            envelope = self._read_json(path)
            if envelope is None:
                continue
            entries.append(
                (
                    path,
                    float(envelope.get("lastAccessedAt") or 0.0),
                    int(envelope.get("byteSize") or path.stat().st_size),
                )
            )
        entries.sort(key=lambda entry: entry[1], reverse=True)
        total_size = 0
        for index, (path, _last_accessed, byte_size) in enumerate(entries):
            total_size += byte_size
            if index >= self.max_entries or total_size > self.max_total_bytes:
                self._delete_quietly(path)

    def _artifact_path(self, cache_key: str) -> Path:
        """Return the artifact envelope path for one hash key."""

        return self.artifact_root / f"{cache_key}.json"

    def _selection_path(self, cache_key: str) -> Path:
        """Return the selection envelope path for one hash key."""

        return self.selection_root / f"{cache_key}.json"

    def _read_json(self, path: Path) -> dict[str, Any] | None:
        """Read a cache envelope, deleting corrupt entries as misses."""

        if not path.exists():
            return None
        try:
            raw = path.read_text(encoding="utf-8")
            value = json.loads(raw)
        except (OSError, json.JSONDecodeError):
            self._delete_quietly(path)
            return None
        if (
            not isinstance(value, dict)
            or value.get("schemaVersion") != self._SCHEMA_VERSION
        ):
            self._delete_quietly(path)
            return None
        return value

    def _write_json_atomic(self, path: Path, value: Mapping[str, Any]) -> None:
        """Write one JSON file with replace-style atomicity."""

        path.parent.mkdir(parents=True, exist_ok=True)
        temp_path = path.with_suffix(f".{os.getpid()}.tmp")
        payload = json.dumps(value, sort_keys=True, separators=(",", ":"))
        temp_path.write_text(payload, encoding="utf-8")
        temp_path.replace(path)

    def _delete_quietly(self, path: Path) -> None:
        """Best-effort delete a stale or corrupt cache file."""

        try:
            path.unlink(missing_ok=True)
        except OSError:
            _logger.debug("Could not delete cube version cache file", exc_info=True)

    def _json_size(self, value: Mapping[str, Any]) -> int:
        """Return the serialized byte size for cache accounting."""

        return len(
            json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
        )

    def _hash_mapping(self, value: Mapping[str, Any]) -> str:
        """Return a SHA-256 hash for one normalized key mapping."""

        payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode(
            "utf-8"
        )
        return hashlib.sha256(payload).hexdigest()
