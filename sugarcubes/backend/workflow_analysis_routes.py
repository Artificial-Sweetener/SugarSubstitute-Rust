#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Adapt canonical Cube graph analysis to the versioned HTTP boundary."""

from __future__ import annotations

from typing import Any, Mapping

from ..execution import CubeExecutionError
from ..workflow import CanonicalWorkflowError
from ..workflow_analysis import CubeGraphAnalysis
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body


def build_workflow_analysis_handler(services: BackendServices) -> RouteHandler:
    """Build a thin dynamic-JSON adapter over canonical graph analysis."""

    async def analyze_workflow(request: Any) -> Any:
        """Return one complete Cube projection without mutating the workflow."""

        try:
            body = await parse_json_body(request)
            if body.get("schema_version") != 1:
                raise BackendError("Unsupported workflow analysis schema", status=400)
            if "workflow" not in body:
                raise BackendError("'workflow' field is required", status=400)
            return json_success(
                workflow_analysis_response(
                    services.workflow_analysis.analyze(body["workflow"])
                )
            )
        except CanonicalWorkflowError as error:
            return json_error_from_exception(
                BackendError(
                    str(error),
                    status=422,
                    extra={"code": error.code, "phase": "workflow", "path": error.path},
                )
            )
        except CubeExecutionError as error:
            return json_error_from_exception(
                BackendError(
                    str(error),
                    status=422,
                    extra={"code": error.code, "phase": error.phase},
                )
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return analyze_workflow


def workflow_analysis_response(analysis: CubeGraphAnalysis) -> Mapping[str, Any]:
    """Project typed analysis into the stable schema-one response."""

    return {
        "schema_version": 1,
        "workflow_semantic_hash": analysis.workflow_semantic_hash,
        "instances": [
            {
                "instance_id": instance.instance_id,
                "node_id": instance.node_id,
                "definition_id": instance.definition_id,
                "cube_id": instance.cube_id,
                "cube_version": instance.cube_version,
                "instance_alias": instance.instance_alias,
                "execution_mode": instance.execution_mode,
            }
            for instance in analysis.instances
        ],
        "edges": [
            {
                "source_instance_id": edge.source_instance_id,
                "source_binding": edge.source_binding,
                "target_instance_id": edge.target_instance_id,
                "target_binding": edge.target_binding,
                "origin": edge.origin.value,
            }
            for edge in analysis.edges
        ],
        "proximity_connections": [
            {
                "source_instance_id": connection.source.instance_id,
                "source_binding": connection.source.binding,
                "target_instance_id": connection.target.instance_id,
                "target_binding": connection.target.binding,
            }
            for connection in analysis.proximity_connections
        ],
        "segments": [
            {
                "instance_ids": list(segment.instance_ids),
                "reorderable": segment.reorderable,
                "boundary_node_ids": list(segment.boundary_node_ids),
            }
            for segment in analysis.segments
        ],
        "workflow": analysis.normalized_workflow,
    }


__all__ = ["build_workflow_analysis_handler", "workflow_analysis_response"]
