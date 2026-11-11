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
"""Verify host loader resolution occurs before validation and queue insertion."""

from __future__ import annotations

import asyncio
from collections.abc import Mapping
from copy import deepcopy
from dataclasses import dataclass, field
from typing import Literal

from sugarcubes.execution.models import ApiPrompt, ExecutionNodeOwner, JsonValue
from sugarcubes.execution.queue_observers import (
    QueueObserverRegistry,
    ValidatedQueueEvent,
)

import pytest

from sugarcubes.execution import CubeExecutionCoordinator, CubeExecutionRequest
from sugarcubes.execution.comfy_execution_port import DirectComfyExecutionPort
from sugarcubes.execution.node_resolvers import ExecutionNodeResolverRegistry
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


@pytest.mark.parametrize("reject", [False, True])
def test_resolved_prompt_is_validated_and_failure_never_queues(reject: bool) -> None:
    """Keep the prepared graph intact and make authorization failures atomic."""
    server = _Server()
    execution = _Validation()
    registry = ExecutionNodeResolverRegistry()

    def resolve(_node: Mapping[str, object]) -> str | None:
        """Select a test runtime implementation or reject authorization."""
        if reject:
            raise ValueError("authorization revoked")
        return "AuthorizedRuntime"

    registry.register(resolve)
    coordinator = CubeExecutionCoordinator()
    prepared = coordinator.prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    original = deepcopy(prepared.prompt)
    port = DirectComfyExecutionPort(
        prompt_server=server,
        execution_module=execution,
        node_resolvers=registry,
    )
    receipt = asyncio.run(port.queue(prepared))
    assert prepared.prompt == original
    assert receipt.accepted is not reject
    if reject:
        assert receipt.execution_prompt == original
        assert execution.seen == []
        assert server.prompt_queue.items == []
        assert receipt.error == {
            "type": "sugarcubes_execution_node_resolution_failed",
            "message": "A host integration could not resolve an execution node.",
        }
    else:
        assert len(execution.seen) == 1
        assert receipt.execution_prompt == execution.seen[0]
        assert server.prompt_queue.items[0][2] == execution.seen[0]
        assert receipt.execution_prompt is not None
        for node_id, node in receipt.execution_prompt.items():
            assert node["class_type"] == "AuthorizedRuntime"
            assert node["inputs"] == original[node_id]["inputs"]


@dataclass
class _Queue:
    """Capture only observable queue insertion."""

    items: list[tuple[object, ...]] = field(default_factory=list)

    def put(self, item: object) -> None:
        """Retain the actual submitted queue tuple."""
        assert isinstance(item, tuple)
        self.items.append(item)


class _Replacements:
    """Model Comfy's independent replacement boundary."""

    def apply_replacements(self, prompt: object) -> None:
        """Accept a detached execution prompt."""
        assert isinstance(prompt, dict)


@dataclass
class _Server:
    """Expose the native queue protocol without starting Comfy."""

    number: float = 0
    prompt_queue: _Queue = field(default_factory=_Queue)
    node_replace_manager: _Replacements = field(default_factory=_Replacements)


class _Validation:
    """Record the final prompt seen by native validation."""

    SENSITIVE_EXTRA_DATA_KEYS: tuple[str, ...] = ()

    def __init__(self) -> None:
        """Initialize per-test validation evidence."""
        self.seen: list[object] = []

    async def validate_prompt(
        self, prompt_id: str, prompt: object, partial_execution_list: object
    ) -> tuple[bool, object, object, object]:
        """Accept and retain the exact prompt validated for queueing."""
        self.seen.append(deepcopy(prompt))
        return True, None, ("output",), {}


class _Policy:
    """Observe resolved classes at the existing execution-only saving boundary."""

    def __init__(self, phases: list[str]) -> None:
        """Retain only preparation and retirement evidence."""
        self.phases = phases
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
        """Receive the same runtime classes that native validation will consume."""
        self.phases.append("policy")
        self.prepared.append(prompt_id)
        assert workflow["nodes"] and owners
        assert all(node["class_type"] == "Runtime" for node in prompt.values())
        return deepcopy(prompt)

    def discard(self, prompt_id: str) -> None:
        """Record retirement of each unqueued candidate."""
        self.discarded.append(prompt_id)


@pytest.mark.parametrize("reject_observer", [False, True])
def test_native_replacements_resolve_before_policy_validation_and_required_observers(
    reject_observer: bool,
) -> None:
    """Preserve queue order, receipts and cleanup around runtime class selection."""
    phases: list[str] = []

    class Replacements(_Replacements):
        """Normalize the external host class before any resolver can inspect it."""

        def apply_replacements(self, prompt: object) -> None:
            """Stamp native normalization on the detached prompt."""
            assert isinstance(prompt, dict)
            phases.append("replacements")
            for node in prompt.values():
                node["class_type"] = "Replaced"

    class Validation(_Validation):
        """Observe policy and resolver order at the external native validator."""

        async def validate_prompt(
            self, prompt_id: str, prompt: object, partial_execution_list: object
        ) -> tuple[bool, object, object, object]:
            """Record exactly one final validation after saving policy preparation."""
            phases.append("validation")
            return await super().validate_prompt(
                prompt_id, prompt, partial_execution_list
            )

    class Queue(_Queue):
        """Observe the actual insertion after required observers accept."""

        def put(self, item: object) -> None:
            """Retain a native tuple only at the final queue boundary."""
            phases.append("queue")
            super().put(item)

    def resolve(node: Mapping[str, object]) -> str | None:
        """Select classes only from the host-normalized node snapshot."""
        assert node["class_type"] == "Replaced"
        phases.append("resolver")
        return "Runtime"

    def observe(event: ValidatedQueueEvent) -> None:
        """See the validated resolved graph and optionally reject the entire insertion."""
        phases.append("observer")
        assert all(node["class_type"] == "Runtime" for node in event.prompt.values())
        if reject_observer:
            raise ValueError("required observer rejected")

    registry, observers = ExecutionNodeResolverRegistry(), QueueObserverRegistry()
    registry.register(resolve)
    observers.register(observe, required=True)
    server = _Server(prompt_queue=Queue(), node_replace_manager=Replacements())
    policy, execution = _Policy(phases), Validation()
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    original = deepcopy(prepared)
    receipt = asyncio.run(
        DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=execution,
            node_resolvers=registry,
            prompt_policy=policy,
            observers=observers,
        ).queue(prepared)
    )
    assert prepared == original
    assert phases == [
        "replacements",
        *(["resolver"] * len(prepared.prompt)),
        "policy",
        "validation",
        "observer",
        *([] if reject_observer else ["queue"]),
    ]
    assert receipt.accepted is not reject_observer
    assert receipt.execution_prompt == execution.seen[0]
    assert policy.prepared == [receipt.prompt_id]
    assert policy.discarded == ([receipt.prompt_id] if reject_observer else [])
    if reject_observer:
        assert server.prompt_queue.items == []
    else:
        assert server.prompt_queue.items[0][2] == receipt.execution_prompt


@pytest.mark.parametrize("failure", ["exception", "conflict", "invalid"])
def test_resolution_rejection_is_atomic_and_retires_the_current_policy_lifetime(
    failure: Literal["exception", "conflict", "invalid"],
) -> None:
    """Skip policy, validation, observers and insertion for every resolution failure."""
    phases: list[str] = []
    registry = ExecutionNodeResolverRegistry()
    visited: list[str] = []

    def resolve(node: Mapping[str, object]) -> str | None:
        """Allow an earlier node before rejecting a later one, proving atomicity."""
        visited.append(str(node["class_type"]))
        if len(visited) == 1:
            return "Runtime"
        if failure == "exception":
            raise ValueError("authorization revoked")
        return "Runtime" if failure == "conflict" else " padded "

    registry.register(resolve)
    if failure == "conflict":
        registry.register(lambda _node: "Different")
    server, execution, policy = _Server(), _Validation(), _Policy(phases)
    observers = QueueObserverRegistry()
    observers.register(lambda _event: phases.append("observer"), required=True)
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    original = deepcopy(prepared)
    receipt = asyncio.run(
        DirectComfyExecutionPort(
            prompt_server=server,
            execution_module=execution,
            node_resolvers=registry,
            prompt_policy=policy,
            observers=observers,
        ).queue(prepared)
    )
    assert not receipt.accepted
    assert receipt.error == {
        "type": "sugarcubes_execution_node_resolution_failed",
        "message": "A host integration could not resolve an execution node.",
    }
    assert receipt.execution_prompt == original.prompt
    assert prepared == original
    assert execution.seen == []
    assert server.prompt_queue.items == []
    assert policy.prepared == []
    assert phases == []
    assert policy.discarded == [receipt.prompt_id]
    assert server.number == 0


def test_no_resolver_preserves_the_detached_native_queue_snapshot() -> None:
    """Keep existing direct execution behavior when no host selects runtime classes."""
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=cube_workflow({"a": provider_document()}))
    )
    server, execution = _Server(), _Validation()
    receipt = asyncio.run(
        DirectComfyExecutionPort(
            prompt_server=server, execution_module=execution
        ).queue(prepared)
    )
    assert receipt.accepted and receipt.execution_prompt == prepared.prompt
    assert (
        receipt.execution_prompt == execution.seen[0] == server.prompt_queue.items[0][2]
    )
    assert receipt.execution_prompt is not prepared.prompt
