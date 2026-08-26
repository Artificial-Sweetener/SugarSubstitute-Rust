#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Build memoized recursive identities for safe Cube-owned prompt outputs."""

from __future__ import annotations

from .optimization_context import (
    NodeSignature,
    OutputAddress,
    PromptOptimizationContext,
    ResourceSignature,
    literal_inputs,
    node_options,
)
from .optimization_policy import PromptTypeClass, ResourceOptimizationPolicy


class GraphSignatureBuilder:
    """Build structural signatures without crossing Cube ownership barriers."""

    def __init__(
        self,
        context: PromptOptimizationContext,
        policy: ResourceOptimizationPolicy,
    ) -> None:
        """Bind the optimization context and its policy."""

        self._context = context
        self._policy = policy

    def signature_for_output(
        self,
        node_id: str,
        output_slot: int,
        *,
        visiting: set[OutputAddress] | None = None,
    ) -> ResourceSignature:
        """Return one deterministic output signature with cycle protection."""

        address = (node_id, output_slot)
        cached = self._context.resource_signature_memo.get(address)
        if cached is not None:
            return cached
        active = visiting if visiting is not None else set()
        if address in active:
            raise ValueError(
                f"Cycle detected while signing prompt output {node_id}:{output_slot}."
            )
        active.add(address)
        signature = self._build_signature(node_id, output_slot, active)
        active.remove(address)
        self._context.resource_signature_memo[address] = signature
        return signature

    def _build_signature(
        self, node_id: str, output_slot: int, visiting: set[OutputAddress]
    ) -> ResourceSignature:
        """Build one uncached resource or value output signature."""

        node = self._context.node(node_id)
        class_type = self._context.class_type(node_id)
        output_type = self._context.output_type(node_id, output_slot)
        output_class = self._policy.output_type_class(output_type)
        if (
            node is None
            or class_type is None
            or output_class
            not in {PromptTypeClass.RESOURCE, PromptTypeClass.PURE_VALUE}
            or not self._policy.can_sign_output(self._context, node_id, output_slot)
        ):
            return self._barrier(node_id, output_slot, output_type)
        linked_inputs = self._context.linked_input_sources(node_id)
        if not linked_inputs:
            if not self._policy.root_identity_is_visible(self._context, node_id):
                return self._barrier(node_id, output_slot, output_type)
            return ResourceSignature(
                value=(
                    self._signature_kind(output_class, root=True),
                    class_type,
                    ("outputSlot", output_slot),
                    ("outputType", output_type),
                    ("options", node_options(node)),
                    (
                        "literals",
                        literal_inputs(self._context.inputs_for_node(node_id)),
                    ),
                ),
                output_type=output_type,
                is_root=output_class is PromptTypeClass.RESOURCE,
            )
        value: NodeSignature = (
            self._signature_kind(output_class, root=False),
            class_type,
            ("outputSlot", output_slot),
            ("outputType", output_type),
            ("options", node_options(node)),
            ("literals", literal_inputs(self._context.inputs_for_node(node_id))),
            ("links", self._linked_signatures(node_id, visiting)),
        )
        return ResourceSignature(value=value, output_type=output_type)

    def _linked_signatures(
        self, node_id: str, visiting: set[OutputAddress]
    ) -> tuple[tuple[str, tuple[object, ...]], ...]:
        """Return recursive upstream signatures or opaque identity barriers."""

        result: list[tuple[str, tuple[object, ...]]] = []
        owner_scope = self._context.scope_key(node_id)
        for name, source_id, output_slot in self._context.linked_input_sources(node_id):
            if self._context.scope_key(source_id) != owner_scope:
                result.append((name, ("identity", source_id, output_slot)))
                continue
            upstream = self.signature_for_output(
                source_id, output_slot, visiting=visiting
            )
            value = (
                ("identity", source_id, output_slot, upstream.output_type)
                if upstream.is_barrier
                else ("signature", upstream.value)
            )
            result.append((name, value))
        return tuple(sorted(result))

    @staticmethod
    def _signature_kind(output_class: PromptTypeClass, *, root: bool) -> str:
        """Return a stable signature namespace."""

        if output_class is PromptTypeClass.PURE_VALUE:
            return "pure_value_root_output" if root else "pure_value_output"
        return "resource_root_output" if root else "resource_output"

    @staticmethod
    def _barrier(
        node_id: str, output_slot: int, output_type: str | None
    ) -> ResourceSignature:
        """Return an opaque identity that cannot merge structurally."""

        return ResourceSignature(
            value=("barrier_output", node_id, output_slot, output_type),
            output_type=output_type,
            is_barrier=True,
        )
