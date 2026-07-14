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
"""Persist machine-local cube identity redirects and promotion provenance."""

from __future__ import annotations

from datetime import datetime, timezone
import json
import logging
import os
from pathlib import Path
from typing import Any, Mapping
from uuid import uuid4

try:
    from ...cube_model import CubeIdentityError, parse_canonical_cube_id
    from ..responses import BackendError
    from .tracked_repo_service import TrackedRepoService
except ImportError:
    from cube_model import CubeIdentityError, parse_canonical_cube_id
    from backend.responses import BackendError
    from backend.services.tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_SCHEMA_VERSION = 1
_PENDING_SCHEMA_VERSION = 1


class CubeIdentityRedirectService:
    """Own durable old-to-new cube identity resolution for this installation."""

    def __init__(self, tracked_repo_service: TrackedRepoService) -> None:
        """Initialize the redirect store below the managed SugarCubes data root."""

        self.tracked_repo_service = tracked_repo_service

    def store_path(self) -> Path:
        """Return the canonical machine-local redirect file path."""

        return self.tracked_repo_service.data_root() / "identity-redirects.json"

    def pending_store_path(self) -> Path:
        """Return the recoverable promotion-operation journal path."""

        return self.tracked_repo_service.data_root() / "promotion-operations.json"

    def resolve(self, cube_id: str) -> str:
        """Resolve redirect chains while rejecting cycles and invalid identities."""

        current = self._canonical_cube_id(cube_id)
        redirects = self._read_redirects()
        visited: set[str] = set()
        while current in redirects:
            if current in visited:
                raise BackendError("Cube identity redirect cycle detected", status=500)
            visited.add(current)
            target = redirects[current].get("target_cube_id")
            current = self._canonical_cube_id(target)
        return current

    def get(self, cube_id: str) -> dict[str, Any] | None:
        """Return one exact redirect record when present."""

        canonical_cube_id = self._canonical_cube_id(cube_id)
        record = self._read_redirects().get(canonical_cube_id)
        return dict(record) if isinstance(record, Mapping) else None

    def begin_promotion(
        self,
        *,
        source_cube_id: str,
        target_cube_id: str,
        source_relative_path: str,
        version: str,
    ) -> dict[str, Any]:
        """Journal promotion intent before mutating either managed cube repository."""

        source_id = self._canonical_cube_id(source_cube_id)
        operation = {
            "target_cube_id": self._canonical_cube_id(target_cube_id),
            "source_relative_path": str(source_relative_path or "").replace("\\", "/"),
            "version": str(version or "").strip(),
            "started_at": datetime.now(timezone.utc).isoformat(),
        }
        operations = self._read_pending_operations()
        existing = operations.get(source_id)
        if existing and existing.get("target_cube_id") != operation["target_cube_id"]:
            raise BackendError(
                "Personal cube already has a pending move to a different destination",
                status=409,
            )
        operations[source_id] = operation
        self._write_pending_operations(operations)
        return {"source_cube_id": source_id, **operation}

    def get_pending(self, cube_id: str) -> dict[str, Any] | None:
        """Return one recoverable promotion operation when present."""

        source_id = self._canonical_cube_id(cube_id)
        operation = self._read_pending_operations().get(source_id)
        return dict(operation) if isinstance(operation, Mapping) else None

    def clear_pending(self, cube_id: str) -> None:
        """Remove one promotion journal entry after rollback or durable redirect creation."""

        source_id = self._canonical_cube_id(cube_id)
        operations = self._read_pending_operations()
        if operations.pop(source_id, None) is not None:
            self._write_pending_operations(operations)

    def record_promotion(
        self,
        *,
        source_cube_id: str,
        target_cube_id: str,
        source_relative_path: str,
        source_commit_sha: str,
        target_commit_sha: str,
        version: str,
    ) -> dict[str, Any]:
        """Persist one completed promotion redirect and its local history anchors."""

        source_id = self._canonical_cube_id(source_cube_id)
        target_id = self._canonical_cube_id(target_cube_id)
        if source_id == target_id:
            raise BackendError(
                "Cube redirect source and target must differ", status=400
            )
        redirects = self._read_redirects()
        record = {
            "target_cube_id": target_id,
            "source_relative_path": str(source_relative_path or "").replace("\\", "/"),
            "source_commit_sha": str(source_commit_sha or "").strip(),
            "target_commit_sha": str(target_commit_sha or "").strip(),
            "version": str(version or "").strip(),
            "promoted_at": datetime.now(timezone.utc).isoformat(),
        }
        redirects[source_id] = record
        self._write_redirects(redirects)
        self.clear_pending(source_id)
        return {"source_cube_id": source_id, **record}

    def _read_pending_operations(self) -> dict[str, dict[str, Any]]:
        """Read normalized recoverable promotion operations."""

        path = self.pending_store_path()
        if not path.exists():
            return {}
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise BackendError(
                "Promotion operation journal is invalid", status=500
            ) from exc
        raw_operations = (
            payload.get("operations") if isinstance(payload, Mapping) else None
        )
        if not isinstance(raw_operations, Mapping):
            raise BackendError("Promotion operation journal is invalid", status=500)
        operations: dict[str, dict[str, Any]] = {}
        for source, operation in raw_operations.items():
            if not isinstance(operation, Mapping):
                continue
            source_id = self._canonical_cube_id(source)
            operations[source_id] = {
                **dict(operation),
                "target_cube_id": self._canonical_cube_id(
                    operation.get("target_cube_id")
                ),
            }
        return operations

    def _write_pending_operations(
        self, operations: Mapping[str, Mapping[str, Any]]
    ) -> None:
        """Atomically persist the recoverable promotion journal."""

        self._write_json_file(
            self.pending_store_path(),
            {
                "schema_version": _PENDING_SCHEMA_VERSION,
                "operations": dict(operations),
            },
            failure_message="Failed to persist promotion operation journal",
        )

    def _read_redirects(self) -> dict[str, dict[str, Any]]:
        """Read normalized redirect records or return an empty store."""

        path = self.store_path()
        if not path.exists():
            return {}
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise BackendError(
                "Cube identity redirect store is invalid", status=500
            ) from exc
        raw_redirects = (
            payload.get("redirects") if isinstance(payload, Mapping) else None
        )
        if not isinstance(raw_redirects, Mapping):
            raise BackendError("Cube identity redirect store is invalid", status=500)
        normalized: dict[str, dict[str, Any]] = {}
        for source, record in raw_redirects.items():
            if not isinstance(record, Mapping):
                continue
            source_id = self._canonical_cube_id(source)
            target_id = self._canonical_cube_id(record.get("target_cube_id"))
            normalized[source_id] = {**dict(record), "target_cube_id": target_id}
        return normalized

    def _write_redirects(self, redirects: Mapping[str, Mapping[str, Any]]) -> None:
        """Atomically persist the complete redirect mapping."""

        self._write_json_file(
            self.store_path(),
            {"schema_version": _SCHEMA_VERSION, "redirects": dict(redirects)},
            failure_message="Failed to persist cube identity redirect",
        )

    def _write_json_file(
        self, path: Path, payload: Mapping[str, Any], *, failure_message: str
    ) -> None:
        """Atomically persist one machine-local identity state document."""

        path.parent.mkdir(parents=True, exist_ok=True)
        temp_path = path.with_name(f"{path.name}.{uuid4().hex}.tmp")
        try:
            temp_path.write_text(
                json.dumps(dict(payload), indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            os.replace(temp_path, path)
        except (OSError, TypeError, ValueError) as exc:
            _logger.exception(
                "SugarCubes: failed to persist identity state '%s'", path.name
            )
            raise BackendError(failure_message, status=500) from exc
        finally:
            try:
                if temp_path.exists():
                    temp_path.unlink()
            except OSError:
                _logger.warning(
                    "SugarCubes: failed to remove redirect temp file",
                    exc_info=True,
                )

    def _canonical_cube_id(self, value: Any) -> str:
        """Return one canonical cube identity or raise an HTTP-safe error."""

        try:
            return parse_canonical_cube_id(str(value or "")).to_string()
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
