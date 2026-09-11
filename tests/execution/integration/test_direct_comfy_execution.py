#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify direct Comfy validation and queueing without prompt hooks."""

from __future__ import annotations

import asyncio
import uuid
from dataclasses import dataclass, field

from sugarcubes.execution import (
    CubeExecutionCoordinator,
    CubeExecutionRequest,
    CubeOptimizationOptions,
    QueueMetadata,
)
from sugarcubes.execution.comfy_execution_port import DirectComfyExecutionPort
from tests.execution.support.execution_fixtures import (
    cube_document,
    cube_workflow,
    provider_document,
)


def test_queue_validates_instrumented_prompt_once_and_preserves_metadata() -> None:
    """Queue one final prompt with native Comfy tuple semantics and no hook dependency."""

    server = _PromptServer(number=8)
    execution = _Execution(valid=(True, None, ["output"], {}))
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=execution,
            uuid_factory=lambda: uuid.UUID(int=1),
            time_source=lambda: 12.5,
        )
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(
                workflow=cube_workflow({"cube-a": provider_document()}),
                queue=QueueMetadata(client_id="client", front=True),
                extra_data={"secret": "kept-private", "caller": "value"},
            )
        )
    )

    assert result.receipt.accepted is True
    assert result.receipt.number == -8
    assert len(execution.calls) == 1
    queued = server.prompt_queue.items[0]
    assert queued[0] == -8
    assert queued[1] == result.receipt.prompt_id
    queued_prompt = queued[2]
    assert isinstance(queued_prompt, dict)
    assert any(
        node.get("class_type") == "SugarCubes.CubeOutput"
        for node in queued_prompt.values()
        if isinstance(node, dict)
    )
    assert queued[3] == {"caller": "value", "client_id": "client", "create_time": 12500}
    assert queued[5] == {"secret": "kept-private"}
    assert server.node_replace_manager.calls == 1


def test_validation_failure_queues_nothing_atomically() -> None:
    """Return Comfy validation diagnostics and leave the queue untouched."""

    server = _PromptServer()
    execution = _Execution(valid=(False, {"type": "invalid"}, (), {"node": "bad"}))
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=execution,
            uuid_factory=lambda: uuid.UUID(int=2),
        )
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(
                workflow=cube_workflow({"cube-a": provider_document()}),
                optimization=CubeOptimizationOptions(enabled=False),
            )
        )
    )

    assert result.receipt.accepted is False
    assert result.receipt.error == {"type": "invalid"}
    assert server.prompt_queue.items == []


def test_queue_wraps_atomic_list_widget_values_before_comfy_validation() -> None:
    """Prevent multiselect widget values from being mistaken for prompt links."""

    server = _PromptServer()
    execution = _Execution(valid=(True, None, (), {}))
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=execution,
            uuid_factory=lambda: uuid.UUID(int=3),
        )
    )
    document = cube_document(
        "Mask List",
        nodes={
            "source": {"class_type": "MaskSource", "inputs": {}},
            "masks": {
                "class_type": "MaskBatch",
                "inputs": {
                    "image": ["01_left_third_hard.png"],
                    "mask": ["source", 0],
                },
            },
        },
        inputs={},
        outputs={"output.mask": ["masks", 0]},
        definitions={
            "MaskSource": {"input": {"required": {}}, "output": ["MASK"]},
            "MaskBatch": {
                "input": {
                    "required": {
                        "image": ["LIST"],
                        "mask": ["MASK", {"forceInput": True}],
                    }
                },
                "output": ["MASK"],
            },
        },
    )

    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(workflow=cube_workflow({"cube-a": document}))
        )
    )

    assert result.receipt.accepted is True
    validated_prompt = execution.calls[0][1]
    assert isinstance(validated_prompt, dict)
    assert validated_prompt["cube-a:masks"]["inputs"]["image"] == {
        "__value__": ["01_left_third_hard.png"]
    }
    assert validated_prompt["cube-a:masks"]["inputs"]["mask"] == [
        "cube-a:source",
        0,
    ]


@dataclass
class _PromptQueue:
    """Capture native Comfy queue tuples."""

    items: list[tuple[object, ...]] = field(default_factory=list)

    def put(self, item: object) -> None:
        """Retain one queue tuple with a test-time shape assertion."""

        assert isinstance(item, tuple)
        self.items.append(item)


@dataclass
class _ReplacementManager:
    """Count replacement normalization calls."""

    calls: int = 0

    def apply_replacements(self, prompt: object) -> None:
        """Record one final-prompt normalization."""

        assert isinstance(prompt, dict)
        self.calls += 1


@dataclass
class _PromptServer:
    """Expose the minimal active Comfy queue state."""

    number: float = 0
    prompt_queue: _PromptQueue = field(default_factory=_PromptQueue)
    node_replace_manager: _ReplacementManager = field(
        default_factory=_ReplacementManager
    )


@dataclass
class _Execution:
    """Return one configured Comfy validation tuple."""

    valid: tuple[bool, object, object, object]
    calls: list[tuple[str, object, object]] = field(default_factory=list)

    @property
    def SENSITIVE_EXTRA_DATA_KEYS(self) -> tuple[str, ...]:
        """Return the configured private metadata keys."""

        return ("secret",)

    async def validate_prompt(
        self, prompt_id: str, prompt: object, partial_execution_list: object
    ) -> tuple[bool, object, object, object]:
        """Capture the exact final prompt passed to Comfy validation."""

        self.calls.append((prompt_id, prompt, partial_execution_list))
        return self.valid
