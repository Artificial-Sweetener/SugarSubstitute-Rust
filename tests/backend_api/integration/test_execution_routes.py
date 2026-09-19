#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify the additive direct Cube execution HTTP adapter."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.execution import (
    ComfyQueueReceipt,
    CubeExecutionCoordinator,
)
from sugarcubes.execution.models import PreparedCubeExecution
from sugarcubes.execution.limits import MAX_PROXIMITY_CONNECTIONS
from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    decode_json_response,
)
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


def test_execution_capabilities_publish_stable_routes_and_schema_versions(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Let external graph clients negotiate the complete native queue contract."""

    handlers = build_route_handlers(backend_services_factory(tmp_path))

    response = asyncio.run(handlers.get_execution_capabilities(FakeRequest()))

    assert response.status == 200
    assert decode_json_response(response) == {
        "available": True,
        "schema_version": 1,
        "workflow_schema_version": 1,
        "report_schema_version": 1,
        "queue_observer_api_version": 1,
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


def test_execution_route_queues_once_and_returns_deterministic_report(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Translate dynamic JSON once and preserve the coordinator result."""

    port = _AcceptingPort()
    services = replace(
        backend_services_factory(tmp_path),
        execution=CubeExecutionCoordinator(comfy=port),
    )
    response = asyncio.run(
        build_route_handlers(services).queue_execution(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": cube_workflow({"cube-a": provider_document()}),
                    "proximity_connections": [],
                    "queue": {"atomic": True},
                    "optimization": {"enabled": False},
                    "extra_data": {"caller": "test"},
                }
            )
        )
    )

    assert response.status == 200
    payload = decode_json_response(response)
    assert payload["accepted"] is True
    assert payload["prompt_id"] == "route-proof"
    assert payload["execution_prompt"] == {"queued-node": {"class_type": "Test"}}
    assert payload["report"]["execution_owner"] == "sugarcubes"
    assert payload["report"]["phase_order"][-1] == "instrumentation"
    assert [name for name, _ in payload["report"]["phase_timings_ms"]] == payload[
        "report"
    ]["phase_order"]
    assert len(port.prepared) == 1


def test_execution_route_rejects_invalid_schema_before_queueing(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep malformed external requests out of the execution domain."""

    port = _AcceptingPort()
    services = replace(
        backend_services_factory(tmp_path),
        execution=CubeExecutionCoordinator(comfy=port),
    )
    response = asyncio.run(
        build_route_handlers(services).queue_execution(
            FakeRequest(body={"schema_version": 99, "workflow": {}})
        )
    )

    assert response.status == 400
    assert port.prepared == []


def test_execution_route_reports_invalid_workflow_as_structured_domain_error(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep canonical workflow validation failures out of Comfy's 500 boundary."""

    port = _AcceptingPort()
    services = replace(
        backend_services_factory(tmp_path),
        execution=CubeExecutionCoordinator(comfy=port),
    )

    response = asyncio.run(
        build_route_handlers(services).queue_execution(
            FakeRequest(body={"schema_version": 1, "workflow": {}})
        )
    )

    assert response.status == 422
    error = decode_json_response(response)["error"]
    assert error["code"] == "workflow.invalid_nodes"
    assert error["phase"] == "workflow"
    assert port.prepared == []


def test_execution_route_bounds_proximity_input_before_domain_translation(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Reject oversized dynamic arrays before allocating typed connections."""

    port = _AcceptingPort()
    services = replace(
        backend_services_factory(tmp_path),
        execution=CubeExecutionCoordinator(comfy=port),
    )
    connection = {
        "source_instance_id": "cube-a",
        "source_binding": "output.resource",
        "target_instance_id": "cube-a",
        "target_binding": "input.resource",
    }
    response = asyncio.run(
        build_route_handlers(services).queue_execution(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": cube_workflow({"cube-a": provider_document()}),
                    "proximity_connections": [connection]
                    * (MAX_PROXIMITY_CONNECTIONS + 1),
                }
            )
        )
    )

    assert response.status == 400
    assert "safety limit" in decode_json_response(response)["error"]["message"]
    assert port.prepared == []


class _AcceptingPort:
    """Capture prepared prompts at the final Comfy boundary."""

    def __init__(self) -> None:
        """Initialize an empty call log."""

        self.prepared: list[PreparedCubeExecution] = []

    async def queue(self, prepared: PreparedCubeExecution) -> ComfyQueueReceipt:
        """Accept one prepared prompt with a stable queue receipt."""

        self.prepared.append(prepared)
        return ComfyQueueReceipt(
            True,
            "route-proof",
            3.0,
            node_errors={},
            execution_prompt={"queued-node": {"class_type": "Test"}},
        )
