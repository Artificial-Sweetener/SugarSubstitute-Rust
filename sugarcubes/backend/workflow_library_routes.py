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
"""Adapt workflow-owned Cube library use cases to versioned HTTP routes."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Literal, Mapping, cast

from ..library import (
    CubeDefinitionClassification,
    CubeForkRequest,
    CubeForkResult,
    CubeLibraryClassReport,
    CubeSourceSyncRequest,
    CubeSourceSyncResult,
    CaptureCubeRequest,
    CaptureCubeResult,
)
from ..workflow import CanonicalWorkflowError, read_canonical_workflow
from .composition import BackendServices
from .responses import BackendError, json_error, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body

_logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class WorkflowLibraryRouteHandlers:
    """Collect versioned workflow library handlers for composition."""

    classify_workflow: RouteHandler
    capture_workflow_cube: RouteHandler
    fork_workflow_cube: RouteHandler
    sync_workflow_cube_source: RouteHandler


def build_workflow_library_route_handlers(
    services: BackendServices,
) -> WorkflowLibraryRouteHandlers:
    """Build thin HTTP adapters over typed workflow library owners."""

    async def classify_workflow(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            workflow = read_canonical_workflow(body.get("workflow"))
            report = services.workflow_library.classify_workflow(workflow)
            return json_success(_serialize_classification(report), status=200)
        except CanonicalWorkflowError as exc:
            return _workflow_error(exc)
        except BackendError as exc:
            return json_error_from_exception(exc)

    async def capture_workflow_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            workflow = read_canonical_workflow(body.get("workflow"))
            result = services.workflow_library.capture(
                workflow,
                CaptureCubeRequest(
                    definition_id=_required_string(body, "definition_id"),
                    expected_semantic_hash=_required_string(
                        body, "expected_semantic_hash"
                    ),
                ),
            )
            return json_success(_serialize_capture_result(result), status=200)
        except CanonicalWorkflowError as exc:
            return _workflow_error(exc)
        except BackendError as exc:
            return json_error_from_exception(exc)
        except ValueError as exc:
            return _value_error(exc)
        except OSError:
            _logger.exception("SugarCubes: failed to capture workflow Cube")
            return json_error("Failed to capture workflow Cube", status=500)

    async def fork_workflow_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            workflow = read_canonical_workflow(body.get("workflow"))
            selected = body.get("selected_instance_ids", [])
            if not isinstance(selected, list) or any(
                not isinstance(value, str) for value in selected
            ):
                raise BackendError(
                    "'selected_instance_ids' must be an array of strings",
                    status=400,
                )
            destination = _required_string(body, "destination")
            if destination not in {"local", "authored_pack"}:
                raise BackendError("Fork destination is invalid", status=400)
            result = services.workflow_forks.fork(
                workflow,
                CubeForkRequest(
                    definition_id=_required_string(body, "definition_id"),
                    expected_semantic_hash=_required_string(
                        body, "expected_semantic_hash"
                    ),
                    new_cube_id=_required_string(body, "new_cube_id"),
                    selected_instance_ids=tuple(selected),
                    destination=cast(Literal["local", "authored_pack"], destination),
                ),
            )
            return json_success(_serialize_fork_result(result), status=201)
        except CanonicalWorkflowError as exc:
            return _workflow_error(exc)
        except BackendError as exc:
            return json_error_from_exception(exc)
        except ValueError as exc:
            return _value_error(exc)

    async def sync_workflow_cube_source(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            workflow = read_canonical_workflow(body.get("workflow"))
            result = services.workflow_source_sync.sync(
                workflow,
                CubeSourceSyncRequest(
                    definition_id=_required_string(body, "definition_id"),
                    expected_semantic_hash=_required_string(
                        body, "expected_semantic_hash"
                    ),
                    approved=body.get("approved") is True,
                ),
            )
            return json_success(_serialize_source_sync_result(result), status=200)
        except CanonicalWorkflowError as exc:
            return _workflow_error(exc)
        except BackendError as exc:
            return json_error_from_exception(exc)
        except PermissionError as exc:
            return json_error(str(exc), status=403)
        except ValueError as exc:
            return _value_error(exc)

    return WorkflowLibraryRouteHandlers(
        classify_workflow=classify_workflow,
        capture_workflow_cube=capture_workflow_cube,
        fork_workflow_cube=fork_workflow_cube,
        sync_workflow_cube_source=sync_workflow_cube_source,
    )


def _serialize_classification(report: CubeLibraryClassReport) -> dict[str, Any]:
    """Serialize classifications while normalizing immutable operation sets."""

    return {
        "schema_version": 1,
        "definitions": [_serialize_definition(item) for item in report.definitions],
    }


def _serialize_definition(
    classification: CubeDefinitionClassification,
) -> dict[str, Any]:
    """Serialize one typed classification for web consumers."""

    return {
        "definition_id": classification.definition_id,
        "cube_id": classification.cube_id,
        "cube_version": classification.cube_version,
        "semantic_hash": classification.semantic_hash,
        "instance_ids": list(classification.instance_ids),
        "primary_class": classification.primary_class,
        "access": classification.access,
        "matches": [
            {
                "state": match.state,
                "library_class": match.library_class,
                "access": match.access,
                "semantic_hash": match.semantic_hash,
                "source_ref": match.source_ref,
            }
            for match in classification.matches
        ],
        "divergent_matches": [
            {
                "state": match.state,
                "library_class": match.library_class,
                "access": match.access,
                "semantic_hash": match.semantic_hash,
                "source_ref": match.source_ref,
            }
            for match in classification.divergent_matches
        ],
        "source_available": classification.source_available,
        "permitted_operations": sorted(classification.permitted_operations),
    }


def _serialize_capture_result(result: CaptureCubeResult) -> dict[str, Any]:
    """Serialize capture persistence without introducing fork fields."""

    return {
        "schema_version": 1,
        "cube_id": result.cube_id,
        "cube_version": result.cube_version,
        "semantic_hash": result.semantic_hash,
        "provenance": dict(result.provenance) if result.provenance else None,
        "created": result.created,
        "classification": _serialize_definition(result.classification),
    }


def _serialize_fork_result(result: CubeForkResult) -> dict[str, Any]:
    """Serialize persisted fork identity, lineage, and optional rebind plan."""

    return {
        "schema_version": 1,
        "source_cube_id": result.source_cube_id,
        "source_semantic_hash": result.source_semantic_hash,
        "fork_cube_id": result.fork_cube_id,
        "fork_semantic_hash": result.fork_semantic_hash,
        "fork_definition_id": result.fork_definition_id,
        "lineage": dict(result.lineage),
        "rebinds": [
            {
                "instance_id": rebind.instance_id,
                "definition_id": rebind.definition_id,
                "cube_id": rebind.cube_id,
            }
            for rebind in result.rebinds
        ],
    }


def _serialize_source_sync_result(result: CubeSourceSyncResult) -> dict[str, Any]:
    """Serialize non-destructive source comparison after approved synchronization."""

    return {
        "schema_version": 1,
        "status": result.status,
        "repo_ref": result.repo_ref,
        "embedded_semantic_hash": result.embedded_semantic_hash,
        "source_semantic_hash": result.source_semantic_hash,
    }


def _required_string(body: Mapping[str, object], key: str) -> str:
    """Read one required request string."""

    value = body.get(key)
    normalized = value.strip() if isinstance(value, str) else ""
    if not normalized:
        raise BackendError(f"'{key}' field is required", status=400)
    return normalized


def _workflow_error(error: CanonicalWorkflowError) -> Any:
    """Return one located canonical-workflow diagnostic."""

    return json_error(
        str(error),
        status=400,
        details={"code": error.code, "path": error.path},
    )


def _value_error(error: ValueError) -> Any:
    """Distinguish stale state from invalid operation requests."""

    status = 409 if "changed after classification" in str(error) else 400
    return json_error(str(error), status=status)


__all__ = [
    "WorkflowLibraryRouteHandlers",
    "build_workflow_library_route_handlers",
]
