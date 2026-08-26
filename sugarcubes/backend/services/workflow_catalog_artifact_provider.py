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
from typing import Any, Literal, Mapping, Protocol

from ...cube_model import (
    CubeDocument,
    CubeSchemaError,
    sanitize_authored_defaults_document,
)
from ...instrumentation import log_diagnostic
from ...library import CatalogCubeArtifact
from ...workflow.semantic_hash import semantic_hash

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes workflow catalog diagnostic"


class CatalogSummaryListing(Protocol):
    """Expose source-aware summaries with canonical internal payloads."""

    def list_catalog_cube_summaries(
        self, *, include_disabled: bool, include_internal_payload: bool
    ) -> list[dict[str, Any]]:
        """Return installed artifact summaries."""


class WorkflowCatalogArtifactProvider:
    """Project current library artifacts into canonical semantic candidates."""

    def __init__(self, listing: CatalogSummaryListing) -> None:
        """Bind the existing catalog listing owner."""

        self._listing = listing

    def list_artifacts(self) -> tuple[CatalogCubeArtifact, ...]:
        """Return deterministic valid artifacts without exposing file-byte hashes."""

        artifacts: list[CatalogCubeArtifact] = []
        summaries = self._listing.list_catalog_cube_summaries(
            include_disabled=False,
            include_internal_payload=True,
        )
        for summary in summaries:
            artifact = self._project(summary)
            if artifact is not None:
                artifacts.append(artifact)
        return tuple(sorted(artifacts, key=lambda item: item.source_ref))

    def _project(self, summary: Mapping[str, object]) -> CatalogCubeArtifact | None:
        """Validate one summary and retain only catalog-comparable content."""

        payload = summary.get("_payload")
        if not isinstance(payload, Mapping):
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
                    "cube_id": _read_string(summary.get("cube_id")),
                    "reason": str(exc),
                },
            )
            return None
        source = summary.get("source")
        source_record = source if isinstance(source, Mapping) else {}
        source_kind = _read_string(source_record.get("type"))
        library_class: Literal["local", "synced"] = (
            "local" if source_kind == "local" else "synced"
        )
        return CatalogCubeArtifact(
            cube_id=document.cube_id,
            cube_version=document.version,
            semantic_hash=semantic_hash(document.to_dict()),
            library_class=library_class,
            access="writable" if summary.get("is_writable") is True else "read_only",
            source_ref=_source_ref(source_kind, source_record),
        )


def _source_ref(source_kind: str, source: Mapping[str, object]) -> str:
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
