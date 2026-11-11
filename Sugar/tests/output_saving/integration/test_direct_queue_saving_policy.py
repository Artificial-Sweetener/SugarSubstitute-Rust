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
"""Safeguard saving preparation order, rejection cleanup and native queue observation."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
from dataclasses import dataclass, field
import asyncio
import pytest

from sugarcubes.execution.comfy_execution_port import DirectComfyExecutionPort
from sugarcubes.execution.coordinator import CubeExecutionCoordinator
from sugarcubes.execution.models import (
    ApiPrompt,
    CubeExecutionRequest,
    ExecutionNodeOwner,
    JsonValue,
)
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


@dataclass
class Queue:
    """Record actual native queue insertion arguments at the external boundary."""

    items: list[object] = field(default_factory=list)

    def put(self, item: object) -> None:
        """Capture the queued native tuple without simulating preparation semantics."""
        self.items.append(item)


class Replacements:
    """Expose the host normalization boundary used by the production queue adapter."""

    def apply_replacements(self, prompt: object) -> None:
        """Leave this already-native fixture unchanged."""


@dataclass
class Server:
    """Model only the actual external surfaces required for direct queue insertion."""

    number: float = 1
    prompt_queue: Queue = field(default_factory=Queue)
    node_replace_manager: Replacements = field(default_factory=Replacements)


class Policy:
    """Capture the immutable workflow and stamp a visibly prepared execution sink."""

    def __init__(self) -> None:
        """Keep queue candidates separate from rejected policy identities."""
        self.prepared: list[str] = []
        self.discarded: list[str] = []

    def prepare(
        self,
        prompt_id: str,
        prompt: ApiPrompt,
        *,
        workflow: Mapping[str, object],
        owners: Mapping[str, ExecutionNodeOwner],
        extra_data: Mapping[str, JsonValue],
    ) -> ApiPrompt:
        """Assert serialized workflow availability before native validation is allowed."""
        assert workflow["nodes"] and owners
        self.prepared.append(prompt_id)
        result = deepcopy(prompt)
        for node in result.values():
            if node["class_type"] == "SugarCubes.CubeOutput":
                inputs = node["inputs"]
                assert isinstance(inputs, dict)
                inputs["save_ticket"] = "captured-ticket"
        return result

    def discard(self, prompt_id: str) -> None:
        """Retire only the rejected queue candidate."""
        self.discarded.append(prompt_id)


class Execution:
    """Validate that policy preparation already happened at the native host boundary."""

    SENSITIVE_EXTRA_DATA_KEYS: tuple[str, ...] = ()

    def __init__(self, *, accepted: bool) -> None:
        """Select the observed native validation outcome."""
        self.accepted = accepted

    async def validate_prompt(
        self, prompt_id: str, prompt: object, targets: object
    ) -> tuple[bool, object, object, object]:
        """Observe fresh sink tickets before returning native acceptance or rejection."""
        assert isinstance(prompt, dict)
        sinks = [
            node
            for node in prompt.values()
            if node["class_type"] == "SugarCubes.CubeOutput"
        ]
        assert sinks and all(
            node["inputs"]["save_ticket"] == "captured-ticket" for node in sinks
        )
        return (
            self.accepted,
            None if self.accepted else {"message": "fixture rejection"},
            ["output"],
            {},
        )


def test_direct_queue_applies_policy_before_validation_and_keeps_accepted_snapshot() -> (
    None
):
    """Queue precisely the validated prepared prompt while preserving the user's serialized graph."""
    server, policy = Server(), Policy()
    workflow = cube_workflow({"a": provider_document()})
    original = deepcopy(workflow)
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=Execution(accepted=True),
            prompt_policy=policy,
        )
    )
    result = asyncio.run(coordinator.queue(CubeExecutionRequest(workflow=workflow)))
    assert result.receipt.accepted
    assert policy.prepared == [result.receipt.prompt_id]
    assert policy.discarded == []
    assert len(server.prompt_queue.items) == 1
    assert workflow == original


def test_native_validation_rejection_discards_policy_without_queue_insertion() -> None:
    """Keep rejected preparations out of both the native queue and retained saving state."""
    server, policy = Server(), Policy()
    coordinator = CubeExecutionCoordinator(
        comfy=DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=Execution(accepted=False),
            prompt_policy=policy,
        )
    )
    result = asyncio.run(
        coordinator.queue(
            CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
        )
    )
    assert not result.receipt.accepted
    assert server.prompt_queue.items == []
    assert policy.discarded == [result.receipt.prompt_id]


@pytest.mark.parametrize(
    "failure", [RuntimeError("host validator failed"), asyncio.CancelledError()]
)
def test_failed_or_cancelled_native_validation_retires_the_preparation(
    failure: BaseException,
) -> None:
    """Keep the host's exception and cancellation semantics while retiring unqueued saving tickets."""

    class BrokenExecution(Execution):
        """Raise from the native validation boundary after saving preparation."""

        async def validate_prompt(
            self, prompt_id: str, prompt: object, targets: object
        ) -> tuple[bool, object, object, object]:
            """Simulate an interrupted host validator without changing the native queue."""
            raise failure

    server, policy = Server(), Policy()
    port = DirectComfyExecutionPort(
        prompt_server=server,
        execution_module=BrokenExecution(accepted=True),
        prompt_policy=policy,
    )
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    with pytest.raises(type(failure)):
        asyncio.run(port.queue(prepared))
    assert policy.discarded == policy.prepared
    assert server.prompt_queue.items == []


def test_queue_insertion_failure_retires_the_preparation() -> None:
    """Retire tickets when an accepted validator cannot actually insert into the native queue."""

    class BrokenQueue(Queue):
        """Expose an external insertion failure before any queue tuple is retained."""

        def put(self, item: object) -> None:
            """Reject the host queue write while preserving the original exception."""
            raise RuntimeError("host queue failed")

    server, policy = Server(prompt_queue=BrokenQueue()), Policy()
    port = DirectComfyExecutionPort(
        prompt_server=server,
        execution_module=Execution(accepted=True),
        prompt_policy=policy,
    )
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    with pytest.raises(RuntimeError, match="host queue failed"):
        asyncio.run(port.queue(prepared))
    assert policy.discarded == policy.prepared
    assert server.prompt_queue.items == []
