#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Own mutable optimizer indexes while enforcing Cube execution scope."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from copy import deepcopy
from dataclasses import dataclass
from typing import TypeAlias

from .models import ApiPrompt, ExecutionNodeOwner, NodeDefinitions

FrozenJson: TypeAlias = (
    str
    | int
    | float
    | bool
    | tuple["FrozenJson", ...]
    | tuple[tuple[str, "FrozenJson"], ...]
    | None
)
NodeSignature: TypeAlias = tuple[object, ...]
OutputAddress: TypeAlias = tuple[str, int]


@dataclass(frozen=True)
class ResourceSignature:
    """Store one output signature and its safety classification."""

    value: NodeSignature
    output_type: str | None
    is_barrier: bool = False
    is_root: bool = False


@dataclass(frozen=True)
class NodeDefinitionView:
    """Expose only execution metadata required by optimization policy."""

    output_types: tuple[str, ...]
    output_node: bool
    hidden_inputs: bool


class PromptOptimizationContext:
    """Own one mutable prompt and its immutable Cube ownership boundary."""

    def __init__(
        self,
        prompt: ApiPrompt,
        *,
        node_owners: Mapping[str, ExecutionNodeOwner],
        node_definitions: NodeDefinitions,
        component_by_instance: Mapping[str, int],
        protected_node_ids: frozenset[str] = frozenset(),
    ) -> None:
        """Copy the prompt and bind all policy inputs for one optimization run."""

        self.prompt = deepcopy(prompt)
        self.node_owners = node_owners
        self.node_definitions = node_definitions
        self.component_by_instance = component_by_instance
        self.protected_node_ids = protected_node_ids
        self.resource_signature_memo: dict[OutputAddress, ResourceSignature] = {}

    def ordered_node_ids(self) -> tuple[str, ...]:
        """Return node ids in deterministic numeric-then-lexical order."""

        return tuple(sorted(self.prompt, key=_node_sort_key))

    def node(self, node_id: str) -> dict[str, object] | None:
        """Return one prompt node if it still exists."""

        return self.prompt.get(node_id)

    def class_type(self, node_id: str) -> str | None:
        """Return one node's class type."""

        node = self.node(node_id)
        value = None if node is None else node.get("class_type")
        return value if isinstance(value, str) else None

    def inputs_for_node(self, node_id: str) -> dict[str, object]:
        """Return the mutable input mapping for one prompt node."""

        return node_inputs(self.prompt[node_id])

    def definition_for_node(self, node_id: str) -> NodeDefinitionView | None:
        """Normalize embedded or live Comfy definition metadata for one node."""

        class_type = self.class_type(node_id)
        raw = self.node_definitions.get(node_id)
        if not isinstance(raw, Mapping) and class_type is not None:
            raw = self.node_definitions.get(class_type)
        if not isinstance(raw, Mapping):
            return None
        outputs = raw.get("output")
        output_types = (
            tuple(str(value) for value in outputs)
            if isinstance(outputs, list | tuple)
            else ()
        )
        inputs = raw.get("input")
        hidden_inputs = False
        if isinstance(inputs, Mapping):
            hidden = inputs.get("hidden")
            hidden_inputs = isinstance(hidden, Mapping) and bool(hidden)
        return NodeDefinitionView(
            output_types=output_types,
            output_node=raw.get("output_node") is True,
            hidden_inputs=hidden_inputs,
        )

    def output_type(self, node_id: str, output_slot: int) -> str | None:
        """Return the declared type for one output slot."""

        definition = self.definition_for_node(node_id)
        if definition is None or not 0 <= output_slot < len(definition.output_types):
            return None
        return definition.output_types[output_slot]

    def output_slots(self, node_id: str) -> tuple[int, ...]:
        """Return declared or observed output slots for one node."""

        definition = self.definition_for_node(node_id)
        if definition is not None and definition.output_types:
            return tuple(range(len(definition.output_types)))
        observed: set[int] = set()
        for node in self.prompt.values():
            for value in node_inputs(node).values():
                address = prompt_link_address(value)
                if address is not None and address[0] == node_id:
                    observed.add(address[1])
        return tuple(sorted(observed))

    def linked_input_sources(self, node_id: str) -> tuple[tuple[str, str, int], ...]:
        """Return linked input names and source addresses deterministically."""

        links: list[tuple[str, str, int]] = []
        for name, value in self.inputs_for_node(node_id).items():
            address = prompt_link_address(value)
            if address is not None:
                links.append((name, address[0], address[1]))
        return tuple(sorted(links))

    def scope_key(self, node_id: str) -> int | None:
        """Return the Cube component eligible to share this node, if any."""

        owner = self.node_owners.get(node_id)
        if owner is None or owner.instance_id is None:
            return None
        return self.component_by_instance.get(owner.instance_id)

    def may_share(self, first_node_id: str, second_node_id: str) -> bool:
        """Return whether two nodes may share identity under Cube-only scope."""

        first = self.scope_key(first_node_id)
        return first is not None and first == self.scope_key(second_node_id)

    def is_loose(self, node_id: str) -> bool:
        """Return whether a node has no Cube execution owner."""

        return self.scope_key(node_id) is None

    def is_protected(self, node_id: str) -> bool:
        """Return whether a public boundary requires this node identity to survive."""

        return node_id in self.protected_node_ids

    def has_remaining_references(self, node_id: str) -> bool:
        """Return whether any prompt input still references one node."""

        return any(
            (address := prompt_link_address(value)) is not None
            and address[0] == node_id
            for node in self.prompt.values()
            for value in node_inputs(node).values()
        )

    def has_output_references(self, node_id: str, output_slot: int) -> bool:
        """Return whether consumers reference one exact output."""

        return any(
            is_prompt_link(value) and value == [node_id, output_slot]
            for node in self.prompt.values()
            for value in node_inputs(node).values()
        )

    def replace_node_links(self, duplicate_node_id: str, canonical_node_id: str) -> int:
        """Replace references to one scoped duplicate node."""

        rewrites = 0
        for node in self.prompt.values():
            for name, value in list(node_inputs(node).items()):
                address = prompt_link_address(value)
                if address is not None and address[0] == duplicate_node_id:
                    node_inputs(node)[name] = [canonical_node_id, address[1]]
                    rewrites += 1
        return rewrites

    def replace_output_links(
        self, duplicate_node_id: str, replacements: Mapping[int, object]
    ) -> int:
        """Replace references using slot-specific passthrough values."""

        rewrites = 0
        for node in self.prompt.values():
            for name, value in list(node_inputs(node).items()):
                address = prompt_link_address(value)
                if address is None or address[0] != duplicate_node_id:
                    continue
                replacement = replacements.get(address[1])
                if replacement is not None:
                    node_inputs(node)[name] = deepcopy(replacement)
                    rewrites += 1
        return rewrites

    def replace_output_slot_links(
        self,
        duplicate_node_id: str,
        duplicate_output_slot: int,
        canonical_node_id: str,
        canonical_output_slot: int,
    ) -> int:
        """Rewrite one duplicate output to its same-component canonical output."""

        rewrites = 0
        for node in self.prompt.values():
            for name, value in list(node_inputs(node).items()):
                if value == [duplicate_node_id, duplicate_output_slot]:
                    node_inputs(node)[name] = [canonical_node_id, canonical_output_slot]
                    rewrites += 1
        return rewrites

    def remove_node(self, node_id: str) -> None:
        """Remove one Cube-owned node and clear its cached signatures."""

        del self.prompt[node_id]
        self.resource_signature_memo = {
            address: value
            for address, value in self.resource_signature_memo.items()
            if address[0] != node_id
        }


def is_prompt_link(value: object) -> bool:
    """Return whether a value is a Comfy API output link."""

    return (
        isinstance(value, list)
        and len(value) == 2
        and isinstance(value[0], str)
        and isinstance(value[1], int)
    )


def prompt_link_address(value: object) -> tuple[str, int] | None:
    """Return a narrowed prompt-link address."""

    if not isinstance(value, list) or len(value) != 2:
        return None
    source_id = value[0]
    output_slot = value[1]
    if not isinstance(source_id, str) or not isinstance(output_slot, int):
        return None
    return source_id, output_slot


def node_inputs(node: dict[str, object]) -> dict[str, object]:
    """Return a mutable node input mapping or reject malformed prompt state."""

    inputs = node.setdefault("inputs", {})
    if not isinstance(inputs, dict):
        raise TypeError("Comfy API prompt node has invalid inputs.")
    return inputs


def literal_inputs(inputs: Mapping[str, object]) -> tuple[tuple[str, FrozenJson], ...]:
    """Return normalized literal input values."""

    return tuple(
        sorted(
            (name, freeze_json(value))
            for name, value in inputs.items()
            if not is_prompt_link(value)
        )
    )


def node_options(node: Mapping[str, object]) -> FrozenJson:
    """Return execution-affecting options outside standard prompt fields."""

    ignored = {
        "class_type",
        "inputs",
        "_meta",
        "outputs",
        "output",
        "output_types",
        "input_types",
        "definitions",
    }
    return freeze_json(
        {key: value for key, value in node.items() if key not in ignored}
    )


def freeze_json(value: object) -> FrozenJson:
    """Convert JSON-like values into deterministic hashable signature data."""

    if isinstance(value, Mapping):
        return tuple(
            sorted((str(key), freeze_json(item)) for key, item in value.items())
        )
    if isinstance(value, list | tuple):
        return tuple(freeze_json(item) for item in value)
    if isinstance(value, str | int | float | bool) or value is None:
        return value
    return json.dumps(value, sort_keys=True, default=str)


def signature_hash(signature: NodeSignature) -> str:
    """Return a compact deterministic report hash."""

    return hashlib.sha256(repr(signature).encode()).hexdigest()[:12]


def _node_sort_key(node_id: str) -> tuple[int, int | str]:
    """Sort numeric Comfy ids before stable symbolic execution ids."""

    try:
        return (0, int(node_id))
    except ValueError:
        return (1, node_id)
