#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify the required pre-queue observer contract for sibling extensions."""

from __future__ import annotations

import asyncio
import uuid
from dataclasses import dataclass, field

from sugarcubes.execution import (
    CubeExecutionCoordinator,
    CubeExecutionRequest,
    QueueMetadata,
)
from sugarcubes.execution.comfy_execution_port import DirectComfyExecutionPort
from sugarcubes.execution.queue_observers import (
    QueueObserverRegistry,
    ValidatedQueueEvent,
)
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


def test_required_observer_receives_validated_prompt_before_queue_insertion() -> None:
    """Publish final prompt context before the native queue becomes observable."""

    server = _PromptServer()
    registry = QueueObserverRegistry()
    observed: list[ValidatedQueueEvent] = []

    def observe(event: ValidatedQueueEvent) -> None:
        """Capture the event while asserting that queue insertion has not happened."""

        assert server.prompt_queue.items == []
        observed.append(event)

    registry.register(observe, required=True)
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=_Execution(),
            observers=registry,
            uuid_factory=lambda: uuid.UUID(int=10),
            time_source=lambda: 25.0,
        )
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(
                workflow=cube_workflow({"cube-a": provider_document()}),
                queue=QueueMetadata(client_id="substitute-client"),
                extra_data={
                    "substitute": {
                        "schemaVersion": 1,
                        "workflowId": "workflow-1",
                    }
                },
            )
        )
    )

    assert result.receipt.accepted is True
    assert len(observed) == 1
    event = observed[0]
    assert event.prompt_id == result.receipt.prompt_id
    assert event.client_id == "substitute-client"
    assert event.extra_data["substitute"] == {
        "schemaVersion": 1,
        "workflowId": "workflow-1",
    }
    assert event.report.execution_owner == "sugarcubes"
    assert event.prompt == server.prompt_queue.items[0][2]
    queued_extra_data = server.prompt_queue.items[0][3]
    assert isinstance(queued_extra_data, dict)
    assert queued_extra_data["create_time"] == 25000


def test_required_observer_failure_rejects_before_queue_insertion() -> None:
    """Fail closed when a required context observer cannot accept the final prompt."""

    server = _PromptServer()
    registry = QueueObserverRegistry()

    def reject(_event: ValidatedQueueEvent) -> None:
        """Simulate failure to persist required sibling-extension context."""

        raise RuntimeError("context store unavailable")

    registry.register(reject, required=True)
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=_Execution(),
            observers=registry,
            uuid_factory=lambda: uuid.UUID(int=11),
        )
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(
                workflow=cube_workflow({"cube-a": provider_document()})
            )
        )
    )

    assert result.receipt.accepted is False
    assert result.receipt.number is None
    assert result.receipt.error == {
        "type": "sugarcubes_queue_observer_failed",
        "message": "A required pre-queue observer rejected the execution.",
    }
    assert server.prompt_queue.items == []


def test_optional_observer_failure_isolated_from_native_queueing() -> None:
    """Keep optional diagnostic integrations from widening execution failure scope."""

    server = _PromptServer()
    registry = QueueObserverRegistry()

    def reject(_event: ValidatedQueueEvent) -> None:
        """Simulate an optional observer failure."""

        raise RuntimeError("optional diagnostics unavailable")

    registry.register(reject, required=False)
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=_Execution(),
            observers=registry,
            uuid_factory=lambda: uuid.UUID(int=12),
        )
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(
                workflow=cube_workflow({"cube-a": provider_document()})
            )
        )
    )

    assert result.receipt.accepted is True
    assert len(server.prompt_queue.items) == 1


@dataclass
class _PromptQueue:
    """Capture native Comfy queue tuples."""

    items: list[tuple[object, ...]] = field(default_factory=list)

    def put(self, item: object) -> None:
        """Retain one queue tuple."""

        assert isinstance(item, tuple)
        self.items.append(item)


class _ReplacementManager:
    """Accept final prompt replacement normalization."""

    def apply_replacements(self, prompt: object) -> None:
        """Assert the replacement input is a prompt mapping."""

        assert isinstance(prompt, dict)


@dataclass
class _PromptServer:
    """Expose the active Comfy queue surface used by the execution port."""

    number: float = 0
    prompt_queue: _PromptQueue = field(default_factory=_PromptQueue)
    node_replace_manager: _ReplacementManager = field(
        default_factory=_ReplacementManager
    )


class _Execution:
    """Accept every prompt through the Comfy validation boundary."""

    @property
    def SENSITIVE_EXTRA_DATA_KEYS(self) -> tuple[str, ...]:
        """Return no private metadata keys for this contract test."""

        return ()

    async def validate_prompt(
        self, prompt_id: str, prompt: object, partial_execution_list: object
    ) -> tuple[bool, object, object, object]:
        """Return a successful native validation tuple."""

        _ = prompt_id, prompt, partial_execution_list
        return True, None, ("output",), {}
