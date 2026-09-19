#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Adapt typed Cube execution to the additive version-2 HTTP endpoint."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from enum import Enum
from typing import Any, Mapping

from ..execution import (
    CubeBoundaryEndpoint,
    CubeExecutionError,
    CubeExecutionRequest,
    CubeOptimizationOptions,
    ProximityConnection,
    QueueMetadata,
)
from ..execution.limits import MAX_PROXIMITY_CONNECTIONS
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body
from ..runtime import QUEUE_OBSERVER_API_VERSION


@dataclass(frozen=True)
class ExecutionRouteHandlers:
    """Collect direct SugarCubes execution routes."""

    get_capabilities: RouteHandler
    queue_execution: RouteHandler


def build_execution_route_handlers(services: BackendServices) -> ExecutionRouteHandlers:
    """Build one thin dynamic-JSON adapter over the execution coordinator."""

    async def get_capabilities(_request: Any) -> Any:
        """Publish the versioned native Cube graph execution contract."""

        return json_success(
            {
                "available": True,
                "schema_version": 1,
                "workflow_schema_version": 1,
                "report_schema_version": 1,
                "queue_observer_api_version": QUEUE_OBSERVER_API_VERSION,
                "queue_route": "/sugarcubes/v2/executions/queue",
                "capabilities_route": "/sugarcubes/v2/executions/capabilities",
                "workflow_analysis_route": "/sugarcubes/v2/workflows/analyze",
                "workflow_reorder_route": "/sugarcubes/v2/workflows/reorder",
                "workflow_cube_append_route": "/sugarcubes/v2/workflows/cubes/append",
                "workflow_cube_create_route": "/sugarcubes/v2/workflows/cubes/create",
                "workflow_cube_remove_route": "/sugarcubes/v2/workflows/cubes/remove",
                "workflow_cube_replace_route": "/sugarcubes/v2/workflows/cubes/replace",
                "execution_owner": "sugarcubes",
                "atomic_queueing": True,
                "cube_scoped_optimization": True,
                "validated_queue_observers": True,
            }
        )

    async def queue_execution(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            execution_request = _parse_request(body)
            result = await services.execution.queue(execution_request)
            receipt = result.receipt
            payload = {
                "accepted": receipt.accepted,
                "prompt_id": receipt.prompt_id,
                "number": receipt.number,
                "error": receipt.error,
                "node_errors": receipt.node_errors,
                "execution_prompt": _json_value(receipt.execution_prompt),
                "report": _json_value(asdict(result.report)),
            }
            return json_success(payload, status=200 if receipt.accepted else 400)
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

    return ExecutionRouteHandlers(
        get_capabilities=get_capabilities,
        queue_execution=queue_execution,
    )


def _parse_request(body: Mapping[str, Any]) -> CubeExecutionRequest:
    """Validate one bounded execution request into typed application values."""

    if body.get("schema_version") != 1:
        raise BackendError("Unsupported execution schema version", status=400)
    if "workflow" not in body:
        raise BackendError("'workflow' field is required", status=400)
    return CubeExecutionRequest(
        workflow=body["workflow"],
        proximity_connections=_parse_proximity(body.get("proximity_connections")),
        queue=_parse_queue(body.get("queue")),
        optimization=_parse_optimization(body.get("optimization")),
        extra_data=_mapping(body.get("extra_data")),
    )


def _parse_proximity(value: object) -> tuple[ProximityConnection, ...]:
    """Parse only explicit Cube-to-Cube endpoint records."""

    if value is None:
        return ()
    if not isinstance(value, list):
        raise BackendError("'proximity_connections' must be an array", status=400)
    if len(value) > MAX_PROXIMITY_CONNECTIONS:
        raise BackendError(
            "Proximity connections exceed the execution safety limit of "
            f"{MAX_PROXIMITY_CONNECTIONS} entries.",
            status=400,
        )
    result: list[ProximityConnection] = []
    for entry in value:
        record = _mapping(entry)
        result.append(
            ProximityConnection(
                source=CubeBoundaryEndpoint(
                    _required_string(record, "source_instance_id"),
                    _required_string(record, "source_binding"),
                ),
                target=CubeBoundaryEndpoint(
                    _required_string(record, "target_instance_id"),
                    _required_string(record, "target_binding"),
                ),
            )
        )
    return tuple(result)


def _parse_queue(value: object) -> QueueMetadata:
    """Parse queue policy while preserving Comfy's supported controls."""

    record = _mapping(value)
    targets = record.get("partial_execution_targets", [])
    if not isinstance(targets, list) or not all(
        isinstance(item, str) for item in targets
    ):
        raise BackendError("Partial execution targets must be strings", status=400)
    number = record.get("number")
    if number is not None and (
        isinstance(number, bool) or not isinstance(number, int | float)
    ):
        raise BackendError("Queue number must be numeric", status=400)
    client_id = record.get("client_id")
    if client_id is not None and not isinstance(client_id, str):
        raise BackendError("Client id must be a string", status=400)
    return QueueMetadata(
        client_id=client_id,
        front=record.get("front") is True,
        number=float(number) if number is not None else None,
        partial_execution_targets=tuple(targets),
        atomic=record.get("atomic", True) is not False,
    )


def _parse_optimization(value: object) -> CubeOptimizationOptions:
    """Parse diagnostic pass switches without exposing scope widening."""

    record = _mapping(value)
    return CubeOptimizationOptions(
        enabled=record.get("enabled", True) is not False,
        bypass_empty_lazy_lora=record.get("bypass_empty_lazy_lora", True) is not False,
        intern_pure_values=record.get("intern_pure_values", True) is not False,
        intern_resource_streams=record.get("intern_resource_streams", True)
        is not False,
    )


def _mapping(value: object) -> dict[str, Any]:
    """Copy one external object or use an empty optional record."""

    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise BackendError("Expected a JSON object", status=400)
    return {str(key): item for key, item in value.items()}


def _required_string(record: Mapping[str, object], name: str) -> str:
    """Read one non-empty endpoint identifier."""

    value = record.get(name)
    if not isinstance(value, str) or not value.strip():
        raise BackendError(f"'{name}' must be a non-empty string", status=400)
    return value.strip()


def _json_value(value: object) -> Any:
    """Convert dataclass output enums and nested containers to JSON values."""

    if isinstance(value, Enum):
        return value.value
    if isinstance(value, Mapping):
        return {str(key): _json_value(item) for key, item in value.items()}
    if isinstance(value, tuple | list):
        return [_json_value(item) for item in value]
    return value
