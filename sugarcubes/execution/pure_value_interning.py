#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Intern allowlisted pure prompt nodes within Cube topology scope."""

from __future__ import annotations

from .models import OptimizationReplacement
from .optimization_context import (
    NodeSignature,
    PromptOptimizationContext,
    literal_inputs,
    node_inputs,
    node_options,
    prompt_link_address,
    signature_hash,
)
from .optimization_registry import (
    evaluate_string_output,
    is_allowlisted_node_class,
    optimization_kind,
    should_preserve_when_string_eval_fails,
)


class PureValueInterner:
    """Own recursive signatures and rewrites for the explicit pure-node registry."""

    def __init__(self, context: PromptOptimizationContext) -> None:
        """Bind one optimizer context."""

        self._context = context

    def run(self, replacements: list[OptimizationReplacement]) -> None:
        """Intern equivalent allowlisted nodes per non-loose Cube component."""

        canonical: dict[tuple[int, NodeSignature], str] = {}
        memo: dict[str, NodeSignature | None] = {}
        for node_id in self._context.ordered_node_ids():
            scope = self._context.scope_key(node_id)
            if scope is None or self._context.node(node_id) is None:
                continue
            signature = self._signature(node_id, memo, set())
            if signature is None:
                continue
            key = (scope, signature)
            canonical_id = canonical.get(key)
            if canonical_id is None or self._context.node(canonical_id) is None:
                canonical[key] = node_id
                continue
            node = self._context.node(node_id)
            if (
                node is None
                or self._context.is_protected(node_id)
                or not self._context.may_share(node_id, canonical_id)
            ):
                continue
            self._context.replace_node_links(node_id, canonical_id)
            if self._context.has_remaining_references(node_id):
                continue
            class_type = str(node.get("class_type", ""))
            self._context.remove_node(node_id)
            replacements.append(
                OptimizationReplacement(
                    kind=optimization_kind(class_type),
                    class_type=class_type,
                    duplicate_node_id=node_id,
                    canonical_node_id=canonical_id,
                    signature_hash=signature_hash(signature),
                )
            )

    def _signature(
        self,
        node_id: str,
        memo: dict[str, NodeSignature | None],
        visiting: set[str],
    ) -> NodeSignature | None:
        """Return one recursive allowlisted signature inside ownership scope."""

        if node_id in memo:
            return memo[node_id]
        if node_id in visiting or self._context.is_loose(node_id):
            return None
        node = self._context.node(node_id)
        if node is None:
            memo[node_id] = None
            return None
        class_type = node.get("class_type")
        if not isinstance(class_type, str) or not is_allowlisted_node_class(class_type):
            memo[node_id] = None
            return None
        visiting.add(node_id)
        evaluated = self._evaluated_string(node_id, visiting)
        if evaluated is None and should_preserve_when_string_eval_fails(node):
            visiting.remove(node_id)
            memo[node_id] = None
            return None
        inputs = node_inputs(node)
        if evaluated is not None and class_type in {
            "PrimitiveString",
            "PrimitiveStringMultiline",
            "RegexExtract",
            "StringConcatenate",
        }:
            literals: tuple[tuple[str, object], ...] = ()
            links: tuple[tuple[str, tuple[object, ...]], ...] = ()
        else:
            literals = literal_inputs(inputs)
            links = self._linked_inputs(node_id, inputs, memo, visiting)
        visiting.remove(node_id)
        signature: NodeSignature = (
            "node",
            class_type,
            ("options", node_options(node)),
            ("evaluatedOutput", evaluated),
            ("literals", literals),
            ("links", links),
        )
        memo[node_id] = signature
        return signature

    def _linked_inputs(
        self,
        owner_id: str,
        inputs: dict[str, object],
        memo: dict[str, NodeSignature | None],
        visiting: set[str],
    ) -> tuple[tuple[str, tuple[object, ...]], ...]:
        """Return recursive inputs with opaque ownership barriers."""

        result: list[tuple[str, tuple[object, ...]]] = []
        for name, value in inputs.items():
            address = prompt_link_address(value)
            if address is None:
                continue
            source_id, output_slot = address
            if not self._context.may_share(owner_id, source_id):
                result.append((name, ("identity", source_id, output_slot)))
                continue
            upstream = self._signature(source_id, memo, visiting)
            value_signature = (
                ("identity", source_id, output_slot)
                if upstream is None
                else ("signature", upstream, output_slot)
            )
            result.append((name, value_signature))
        return tuple(sorted(result))

    def _evaluated_string(self, node_id: str, visiting: set[str]) -> str | None:
        """Evaluate a supported string output through scoped links."""

        node = self._context.node(node_id)
        if node is None:
            return None

        def resolve(source_id: str, output_slot: int) -> str | None:
            if (
                output_slot != 0
                or source_id in visiting
                or not self._context.may_share(node_id, source_id)
            ):
                return None
            source = self._context.node(source_id)
            if source is None:
                return None
            return evaluate_string_output(source, resolve)

        return evaluate_string_output(node, resolve)
