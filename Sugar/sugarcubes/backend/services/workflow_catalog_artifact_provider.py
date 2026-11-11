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
"""Adapt installed Cube summaries to workflow classification candidates."""

from __future__ import annotations

import logging
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Literal, Protocol

from ...cube_model import (
    CubeDocument,
    CubeSchemaError,
    sanitize_authored_defaults_document,
)
from ...instrumentation import log_diagnostic
from ...library import CatalogCubeArtifact
from ...workflow.semantic_hash import semantic_hash
from .cube_file_io import read_cube_payload
from .cube_library_listing import CatalogCubeFile

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes workflow catalog diagnostic"


class CatalogCubeFileListing(Protocol):
    """Discover catalog files and their ownership without reading file contents."""

    def list_catalog_cube_files(
        self, *, include_disabled: bool
    ) -> list[CatalogCubeFile]:
        """Return installed catalog files in listing order."""

    def catalog_file_writable(self, file: CatalogCubeFile) -> bool:
        """Return whether ownership policy permits writing one file's source."""


@dataclass(frozen=True)
class _ArtifactIdentity:
    """Hold the content-derived facts that are expensive to recompute for one file."""

    cube_id: str
    cube_version: str
    semantic_hash: str


@dataclass(frozen=True)
class _CachedIdentity:
    """Bind one file's derived identity, or its rejection, to the stat facts it was read under."""

    stamp: tuple[int, int]
    identity: _ArtifactIdentity | None


class WorkflowCatalogArtifactProvider:
    """Project current library artifacts into canonical semantic candidates.

    Reading, validating and hashing every installed Cube dominated classification. Each file's
    derived identity is cached under its resolved path and reused while its modification time
    and size are unchanged; discovery and ownership still run per call, so added, removed,
    retargeted or re-permissioned files are always current. Calls may run on worker threads.
    """

    def __init__(self, listing: CatalogCubeFileListing) -> None:
        """Bind the existing catalog listing owner."""

        self._listing = listing
        self._identities: dict[Path, _CachedIdentity] = {}
        self._lock = threading.Lock()

    def list_artifacts(self) -> tuple[CatalogCubeArtifact, ...]:
        """Return deterministic valid artifacts without exposing file-byte hashes."""

        artifacts: list[CatalogCubeArtifact] = []
        files = self._listing.list_catalog_cube_files(include_disabled=False)
        live: set[Path] = set()
        for file in files:
            live.add(file.path)
            identity = self._identity(file)
            if identity is not None:
                artifacts.append(self._artifact(file, identity))
        with self._lock:
            for path in [path for path in self._identities if path not in live]:
                del self._identities[path]
        return tuple(sorted(artifacts, key=lambda item: item.source_ref))

    def _identity(self, file: CatalogCubeFile) -> _ArtifactIdentity | None:
        """Return one file's cached identity while its stat facts are unchanged."""

        try:
            stat = file.path.stat()
        except OSError:
            return None
        stamp = (stat.st_mtime_ns, stat.st_size)
        with self._lock:
            cached = self._identities.get(file.path)
        if cached is not None and cached.stamp == stamp:
            return cached.identity
        identity = self._read_identity(file)
        with self._lock:
            self._identities[file.path] = _CachedIdentity(
                stamp=stamp, identity=identity
            )
        return identity

    def _read_identity(self, file: CatalogCubeFile) -> _ArtifactIdentity | None:
        """Validate one file and retain only catalog-comparable content."""

        payload, _error = read_cube_payload(file.path)
        if payload is None:
            return None
        try:
            document = sanitize_authored_defaults_document(
                CubeDocument.from_dict(payload)
            )
        except CubeSchemaError as exc:
            log_diagnostic(
                _logger,
                _TRACE_MARKER,
                "sugarcubes_workflow_catalog_artifact_ignored",
                {
                    "cube_id": _read_string(payload.get("cube_id")),
                    "reason": str(exc),
                },
            )
            return None
        return _ArtifactIdentity(
            cube_id=document.cube_id,
            cube_version=document.version,
            semantic_hash=semantic_hash(document.to_dict()),
        )

    def _artifact(
        self, file: CatalogCubeFile, identity: _ArtifactIdentity
    ) -> CatalogCubeArtifact:
        """Combine cached content identity with current source and ownership facts."""

        library_class: Literal["local", "synced"] = (
            "local" if file.source_kind == "local" else "synced"
        )
        return CatalogCubeArtifact(
            cube_id=identity.cube_id,
            cube_version=identity.cube_version,
            semantic_hash=identity.semantic_hash,
            library_class=library_class,
            access=(
                "writable" if self._listing.catalog_file_writable(file) else "read_only"
            ),
            source_ref=_source_ref(file.source_kind, file.source),
        )


def _source_ref(source_kind: str, source: dict[str, str]) -> str:
    """Build one stable source identifier without leaking absolute paths."""

    relative_path = _read_string(source.get("repo_relative_path"))
    if source_kind == "local":
        namespace = _read_string(source.get("namespace")) or "personal"
        return f"local:{namespace}:{relative_path}"
    repo_ref = _read_string(source.get("repo_ref")) or "unknown"
    return f"github:{repo_ref}:{relative_path}"


def _read_string(value: object) -> str:
    """Normalize optional summary text."""

    return value.strip() if isinstance(value, str) else ""
