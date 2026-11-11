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
"""Load current or historical Cube artifacts into prepared frontend responses."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any, Callable, Sequence

from ...importer import CubeImportError
from ...instrumentation import log_diagnostic
from ..responses import BackendError
from .cube_git_context import CubeGitContext
from .cube_icon_service import attach_icon_url, normalize_existing_icon_metadata
from .cube_metadata import normalize_metadata_string
from .cube_summary import build_cube_identity_fields

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube revision diagnostic"


class CubeRevisionLoader:
    """Own revision artifact validation and prepared-import response shaping."""

    def __init__(
        self,
        *,
        load_cube_artifact: Callable[[Any], Any],
        prepare_cube_import: Callable[..., Any],
    ) -> None:
        """Bind importer ports shared by current and historical revisions."""

        self._load_cube_artifact = load_cube_artifact
        self._prepare_cube_import = prepare_cube_import

    def load_path(
        self,
        cube_path: Path,
        context: CubeGitContext,
        *,
        revision_ref: str,
        current: bool,
        version_pin: str,
        drop_origin: Sequence[float],
    ) -> dict[str, Any]:
        """Load one artifact path and retain portable content in its response."""

        try:
            loaded_cube = self._load_cube_artifact(cube_path)
            if version_pin and loaded_cube.version != version_pin:
                self._log_version_mismatch(
                    context=context,
                    revision_ref=revision_ref,
                    expected=version_pin,
                    actual=loaded_cube.version,
                )
                raise BackendError(
                    "Cube version mismatch",
                    status=409,
                    details={"expected": version_pin, "actual": loaded_cube.version},
                )
            prepared = self._prepare_cube_import(
                loaded_cube,
                drop_origin=drop_origin,
            )
        except CubeImportError:
            raise
        except BackendError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception(
                "SugarCubes: failed to load revision '%s' for cube '%s'",
                revision_ref,
                context.cube_id,
            )
            raise BackendError("Load failed", status=500) from exc

        cube_payload = _build_cube_payload(prepared, context)
        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_revision_load_return",
            {
                "cube_id": context.cube_id,
                "revision_ref": revision_ref,
                "current": current,
                "loaded_cube_id": normalize_metadata_string(
                    cube_payload.get("cube_id")
                ),
                "loaded_version": normalize_metadata_string(
                    cube_payload.get("version")
                ),
            },
        )
        response = {
            "cube": cube_payload,
            "document": dict(loaded_cube.document),
            "nodes": prepared.nodes,
            "markers": prepared.markers,
            "connections": prepared.connections,
            "layout": prepared.layout,
            "warnings": prepared.warnings,
            "subgraphs": prepared.subgraphs,
            "source": _source_payload(context),
            "revision": {"revision_ref": revision_ref, "current": current},
        }
        boundaries = getattr(prepared, "boundaries", None)
        if boundaries is not None:
            response["boundaries"] = boundaries
        return response

    def _log_version_mismatch(
        self,
        *,
        context: CubeGitContext,
        revision_ref: str,
        expected: str,
        actual: object,
    ) -> None:
        """Record a located immutable-revision version mismatch."""

        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_revision_version_pin_mismatch",
            {
                "cube_id": context.cube_id,
                "revision_ref": revision_ref,
                "expected_version": expected,
                "actual_version": actual,
            },
        )


def _build_cube_payload(prepared: Any, context: CubeGitContext) -> dict[str, Any]:
    """Complete prepared metadata with stable identity and icon presentation."""

    cube_payload = dict(prepared.cube)
    cube_payload.setdefault("name", context.cube_path.stem)
    metadata_value = cube_payload.get("metadata")
    metadata = metadata_value if isinstance(metadata_value, dict) else {}
    icon = attach_icon_url(
        normalize_existing_icon_metadata(metadata.get("icon")),
        normalize_metadata_string(cube_payload.get("cube_id")),
    )
    if icon:
        cube_payload["icon"] = icon
    cube_payload.update(
        build_cube_identity_fields(
            cube_id=normalize_metadata_string(cube_payload.get("cube_id"))
            or context.cube_id,
            default_alias=normalize_metadata_string(metadata.get("default_alias"))
            or normalize_metadata_string(cube_payload.get("name"))
            or context.cube_path.stem,
            metadata=metadata,
        )
    )
    return cube_payload


def _source_payload(context: CubeGitContext) -> dict[str, object]:
    """Project source identity without exposing loader implementation details."""

    return {
        "path": str(context.cube_path),
        "name": context.cube_path.stem,
        "type": context.source_kind,
        "owner": context.owner,
        "repo": context.repo,
        "repo_ref": (
            f"{context.owner}/{context.repo}" if context.owner and context.repo else ""
        ),
        "namespace": context.namespace,
        "relative_path": context.repo_relative_path,
    }
