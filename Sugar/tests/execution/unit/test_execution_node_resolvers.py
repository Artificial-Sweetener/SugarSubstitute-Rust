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
"""Verify bounded host substitutions preserve the prepared graph contract."""

from collections.abc import Mapping
from copy import deepcopy

import pytest

from sugarcubes.execution.node_resolvers import (
    ExecutionNodeResolutionError,
    ExecutionNodeResolverRegistry,
)
from sugarcubes.execution.models import ApiPrompt


def test_resolution_changes_only_the_execution_class() -> None:
    """Keep authored values, topology, and unrelated nodes intact."""
    prompt: ApiPrompt = {
        "image": {"class_type": "LoadImage", "inputs": {"image": "test-token"}},
        "consumer": {"class_type": "Consumer", "inputs": {"image": ["image", 0]}},
    }
    original = deepcopy(prompt)
    registry = ExecutionNodeResolverRegistry()

    def resolve(node: Mapping[str, object]) -> str | None:
        """Recognize one authorized image without owning its input values."""
        return "AuthorizedImage" if node["class_type"] == "LoadImage" else None

    registry.register(resolve)
    result = registry.resolve(prompt)
    assert result["image"]["class_type"] == "AuthorizedImage"
    assert result["image"]["inputs"] == original["image"]["inputs"]
    assert result["consumer"] == original["consumer"]
    assert prompt == original


def test_registration_is_idempotent_and_can_be_removed() -> None:
    """Keep extension registration safe across repeated host initialization."""
    calls: list[str] = []

    def resolve(node: Mapping[str, object]) -> str | None:
        """Count resolutions without requesting a replacement."""
        calls.append(str(node["class_type"]))
        return None

    registry = ExecutionNodeResolverRegistry()
    registry.register(resolve)
    registry.register(resolve)
    prompt: ApiPrompt = {"node": {"class_type": "Original", "inputs": {}}}
    assert registry.resolve(prompt) == prompt
    assert calls == ["Original"]
    registry.unregister(resolve)
    assert registry.resolve(prompt) == prompt
    assert calls == ["Original"]


@pytest.mark.parametrize("replacement", ["", "  ", " padded "])
def test_invalid_class_rejected(replacement: str) -> None:
    """Reject malformed extension output before Comfy validation."""
    registry = ExecutionNodeResolverRegistry()
    registry.register(lambda _node: replacement)
    with pytest.raises(ExecutionNodeResolutionError):
        registry.resolve({"node": {"class_type": "Original", "inputs": {}}})


def test_conflicting_resolvers_rejected_without_order_dependence() -> None:
    """Require one unambiguous execution class for each original node."""
    registry = ExecutionNodeResolverRegistry()
    registry.register(lambda _node: "First")
    registry.register(lambda _node: "Second")
    with pytest.raises(ExecutionNodeResolutionError):
        registry.resolve({"node": {"class_type": "Original", "inputs": {}}})


def test_resolver_failure_cannot_fall_through_to_another_loader() -> None:
    """Preserve failed authorization as a rejected execution."""
    registry = ExecutionNodeResolverRegistry()

    def reject(_node: Mapping[str, object]) -> str | None:
        """Simulate a revoked local-file authorization."""
        raise ValueError("authorization revoked")

    registry.register(reject)
    with pytest.raises(ExecutionNodeResolutionError) as failure:
        registry.resolve({"node": {"class_type": "Original", "inputs": {}}})
    assert isinstance(failure.value.__cause__, ValueError)


def test_resolver_cannot_modify_inputs_or_influence_other_resolvers() -> None:
    """Confine sibling integrations to class selection over detached evidence."""
    registry = ExecutionNodeResolverRegistry()
    prompt: ApiPrompt = {
        "node": {"class_type": "Original", "inputs": {"value": "original"}}
    }

    def mutate(node: Mapping[str, object]) -> str | None:
        """Attempt an unsupported mutation of a nested input."""
        inputs = node["inputs"]
        assert isinstance(inputs, dict)
        inputs["value"] = "changed"
        return None

    def observe(node: Mapping[str, object]) -> str | None:
        """Receive the same original node independently of earlier callbacks."""
        assert node["inputs"] == {"value": "original"}
        return None

    registry.register(mutate)
    registry.register(observe)
    assert registry.resolve(prompt) == prompt


def test_equal_selections_agree_and_empty_registry_returns_a_detached_prompt() -> None:
    """Preserve no-integration behavior and permit independent hosts to agree."""
    prompt: ApiPrompt = {"node": {"class_type": "Original", "inputs": {"value": []}}}
    registry = ExecutionNodeResolverRegistry()
    detached = registry.resolve(prompt)
    assert detached == prompt and detached is not prompt
    assert detached["node"]["inputs"] is not prompt["node"]["inputs"]
    registry.register(lambda _node: "Runtime")
    registry.register(lambda _node: "Runtime")
    assert registry.resolve(prompt)["node"]["class_type"] == "Runtime"


def test_registration_changes_apply_only_to_the_next_resolution() -> None:
    """Retain one callback snapshot across every node without holding its lock."""
    registry = ExecutionNodeResolverRegistry()
    seen: list[str] = []

    def next_resolution(node: Mapping[str, object]) -> str | None:
        """Observe only the following execution snapshot."""
        seen.append("next")
        return None

    def current_resolution(node: Mapping[str, object]) -> str | None:
        """Change registration while resolving the current snapshot."""
        seen.append(str(node["class_type"]))
        registry.unregister(current_resolution)
        registry.register(next_resolution)
        return None

    registry.register(current_resolution)
    prompt: ApiPrompt = {
        "one": {"class_type": "First", "inputs": {}},
        "two": {"class_type": "Second", "inputs": {}},
    }
    registry.resolve(prompt)
    assert seen == ["First", "Second"]
    registry.resolve(prompt)
    assert seen == ["First", "Second", "next", "next"]
