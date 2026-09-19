#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Adapt SugarCubes-owned graph mutations to versioned HTTP routes."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from ..execution import CubeExecutionError
from ..workflow import CanonicalWorkflowError
from ..workflow_mutation import CubeGraphDraft, CubeGraphMutationError
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body
from .workflow_analysis_routes import workflow_analysis_response


def build_workflow_reorder_handler(services: BackendServices) -> RouteHandler:
    """Build the atomic Cube-segment reorder adapter."""

    async def reorder_workflow(request: Any) -> Any:
        """Return a normalized graph and refreshed projection after reordering."""

        try:
            body = await _mutation_body(request)
            result = services.workflow_mutation.reorder(
                body["workflow"],
                segment_instance_ids=_string_sequence(body.get("segment_instance_ids")),
                ordered_instance_ids=_string_sequence(body.get("ordered_instance_ids")),
            )
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return _mutation_error(error)

    return reorder_workflow


def build_workflow_append_cube_handler(services: BackendServices) -> RouteHandler:
    """Build the atomic exact-Cube append adapter."""

    async def append_cube(request: Any) -> Any:
        """Return the graph and projection after appending one Cube document."""

        try:
            body = await _mutation_body(request)
            cube = body.get("cube")
            if not isinstance(cube, Mapping):
                raise BackendError("'cube' field must be an object", status=400)
            bypassed = cube.get("bypassed", False)
            if not isinstance(bypassed, bool):
                raise BackendError("Cube bypassed state must be a boolean", status=400)
            result = services.workflow_mutation.append_cube(
                body["workflow"],
                instance_id=_required_string(cube.get("instance_id"), "instance_id"),
                alias=_required_string(cube.get("alias"), "alias"),
                bypassed=bypassed,
                document=cube.get("document"),
            )
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return _mutation_error(error)

    return append_cube


def build_workflow_remove_cube_handler(services: BackendServices) -> RouteHandler:
    """Build the atomic recognized-Cube removal adapter."""

    async def remove_cube(request: Any) -> Any:
        """Return the graph and projection after removing one Cube instance."""

        try:
            body = await _mutation_body(request)
            result = services.workflow_mutation.remove_cube(
                body["workflow"],
                instance_id=_required_string(body.get("instance_id"), "instance_id"),
            )
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return _mutation_error(error)

    return remove_cube


def build_workflow_replace_cube_handler(services: BackendServices) -> RouteHandler:
    """Build the atomic recognized-Cube replacement adapter."""

    async def replace_cube(request: Any) -> Any:
        """Return the graph and projection after replacing one Cube document."""

        try:
            body = await _mutation_body(request)
            result = services.workflow_mutation.replace_cube(
                body["workflow"],
                instance_id=_required_string(body.get("instance_id"), "instance_id"),
                document=body.get("document"),
            )
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return _mutation_error(error)

    return replace_cube


def build_workflow_create_cube_handler(services: BackendServices) -> RouteHandler:
    """Build the one-request legacy Cube-stack migration adapter."""

    async def create_cube_workflow(request: Any) -> Any:
        """Return one canonical native graph from ordered Cube documents."""

        try:
            body = await parse_json_body(request)
            if body.get("schema_version") != 1:
                raise BackendError("Unsupported workflow mutation schema", status=400)
            cubes = body.get("cubes")
            if not isinstance(cubes, list):
                raise BackendError("'cubes' field must be an array", status=400)
            drafts: list[CubeGraphDraft] = []
            for cube in cubes:
                if not isinstance(cube, Mapping):
                    raise BackendError("Cube entry must be an object", status=400)
                bypassed = cube.get("bypassed", False)
                if not isinstance(bypassed, bool):
                    raise BackendError(
                        "Cube bypassed state must be a boolean",
                        status=400,
                    )
                drafts.append(
                    CubeGraphDraft(
                        instance_id=_required_string(
                            cube.get("instance_id"), "instance_id"
                        ),
                        alias=_required_string(cube.get("alias"), "alias"),
                        bypassed=bypassed,
                        document=cube.get("document"),
                    )
                )
            result = services.workflow_mutation.create_cube_workflow(drafts)
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return _mutation_error(error)

    return create_cube_workflow


async def _mutation_body(request: Any) -> Mapping[str, Any]:
    """Validate the shared workflow-mutation request envelope."""

    body = await parse_json_body(request)
    if body.get("schema_version") != 1:
        raise BackendError("Unsupported workflow mutation schema", status=400)
    if "workflow" not in body:
        raise BackendError("'workflow' field is required", status=400)
    return body


def _string_sequence(value: object) -> tuple[str, ...]:
    """Require one non-empty string array at the HTTP boundary."""

    if not isinstance(value, list) or not all(
        isinstance(item, str) and item.strip() for item in value
    ):
        raise BackendError("Workflow mutation instance ids must be strings", status=400)
    return tuple(item.strip() for item in value)


def _required_string(value: object, field: str) -> str:
    """Require one non-empty mutation field."""

    if not isinstance(value, str) or not value.strip():
        raise BackendError(f"Cube {field} must be a non-empty string", status=400)
    return value.strip()


def _mutation_error(error: Exception) -> Any:
    """Preserve structured workflow errors across every mutation route."""

    if isinstance(error, BackendError):
        return json_error_from_exception(error)
    if isinstance(error, CanonicalWorkflowError):
        return json_error_from_exception(
            BackendError(
                str(error),
                status=422,
                extra={"code": error.code, "phase": "workflow", "path": error.path},
            )
        )
    if isinstance(error, CubeExecutionError):
        return json_error_from_exception(
            BackendError(
                str(error),
                status=422,
                extra={"code": error.code, "phase": error.phase},
            )
        )
    return json_error_from_exception(BackendError(str(error), status=422))


__all__ = [
    "build_workflow_append_cube_handler",
    "build_workflow_create_cube_handler",
    "build_workflow_remove_cube_handler",
    "build_workflow_replace_cube_handler",
    "build_workflow_reorder_handler",
]
