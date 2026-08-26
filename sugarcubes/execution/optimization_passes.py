#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Apply the ordered conservative SugarCubes graph rewrite passes."""

from __future__ import annotations

from .models import CubeOptimizationOptions, OptimizationReplacement
from .optimization_context import (
    NodeSignature,
    PromptOptimizationContext,
    freeze_json,
    node_inputs,
    prompt_link_address,
    signature_hash,
)
from .optimization_policy import ResourceOptimizationPolicy
from .optimization_registry import evaluate_string_output
from .optimization_signatures import GraphSignatureBuilder
from .pure_value_interning import PureValueInterner


class CubeOptimizationPasses:
    """Own the fixed pass order and graph mutation details."""

    def __init__(
        self,
        context: PromptOptimizationContext,
        policy: ResourceOptimizationPolicy,
    ) -> None:
        """Bind one mutable optimization context."""

        self._context = context
        self._policy = policy
        self._pure_values = PureValueInterner(context)

    def run(
        self, options: CubeOptimizationOptions
    ) -> tuple[list[OptimizationReplacement], tuple[tuple[str, int], ...]]:
        """Run each enabled pass once in the authoritative sequence."""

        replacements: list[OptimizationReplacement] = []
        counts: list[tuple[str, int]] = []
        passes = (
            (
                "bypass_empty_lazy_lora",
                options.bypass_empty_lazy_lora,
                self._bypass_empty_lora,
            ),
            (
                "intern_pure_values",
                options.intern_pure_values,
                self._pure_values.run,
            ),
            (
                "intern_resource_streams",
                options.intern_resource_streams,
                self._intern_resource_streams,
            ),
        )
        for name, enabled, operation in passes:
            before = len(replacements)
            if enabled:
                operation(replacements)
            counts.append((name, len(replacements) - before))
        return replacements, tuple(counts)

    def _bypass_empty_lora(self, replacements: list[OptimizationReplacement]) -> None:
        """Remove empty Prompt Control LoRA schedulers inside Cube scope."""

        for node_id in self._context.ordered_node_ids():
            node = self._context.node(node_id)
            if (
                node is None
                or self._context.is_loose(node_id)
                or self._context.is_protected(node_id)
                or node.get("class_type")
                not in {"PCLazyLoraLoader", "PCLazyLoraLoaderAdvanced"}
            ):
                continue
            inputs = node_inputs(node)
            if self._resolve_string(inputs.get("text"), {node_id}) != "":
                continue
            model_input = inputs.get("model")
            clip_input = inputs.get("clip")
            if model_input is None or clip_input is None:
                continue
            if not self._lora_inputs_are_scoped(
                node_id, model_input, clip_input, inputs.get("text")
            ) or not self._lora_references_are_scoped(node_id):
                continue
            self._context.replace_output_links(node_id, {0: model_input, 1: clip_input})
            if self._context.has_remaining_references(node_id):
                continue
            class_type = str(node["class_type"])
            self._context.remove_node(node_id)
            replacements.append(
                OptimizationReplacement(
                    kind="empty_lora_passthrough",
                    class_type=class_type,
                    duplicate_node_id=node_id,
                    canonical_node_id=_passthrough_label(model_input, clip_input),
                    signature_hash=signature_hash(
                        (
                            "empty_lora_passthrough",
                            class_type,
                            freeze_json(model_input),
                            freeze_json(clip_input),
                        )
                    ),
                )
            )

    def _intern_resource_streams(
        self, replacements: list[OptimizationReplacement]
    ) -> None:
        """Intern equivalent resource streams within one Cube component."""

        builder = GraphSignatureBuilder(self._context, self._policy)
        canonical: dict[tuple[int, NodeSignature], tuple[str, int]] = {}
        for node_id in self._context.ordered_node_ids():
            scope = self._context.scope_key(node_id)
            if scope is None or self._context.node(node_id) is None:
                continue
            for output_slot in self._context.output_slots(node_id):
                if self._context.node(node_id) is None:
                    break
                eligible, _ = self._policy.intern_output_decision(
                    self._context, node_id, output_slot
                )
                if not eligible:
                    continue
                resource_signature = builder.signature_for_output(node_id, output_slot)
                if resource_signature.is_barrier or resource_signature.is_root:
                    continue
                key = (scope, resource_signature.value)
                canonical_output = canonical.get(key)
                if (
                    canonical_output is None
                    or self._context.node(canonical_output[0]) is None
                ):
                    canonical[key] = (node_id, output_slot)
                    continue
                canonical_id, canonical_slot = canonical_output
                if (
                    not self._context.may_share(node_id, canonical_id)
                    or self._context.is_protected(node_id)
                    or not self._context.has_output_references(node_id, output_slot)
                ):
                    continue
                rewritten = self._context.replace_output_slot_links(
                    node_id,
                    output_slot,
                    canonical_id,
                    canonical_slot,
                )
                if not rewritten:
                    continue
                node = self._context.node(node_id)
                if node is None:
                    continue
                replacements.append(
                    OptimizationReplacement(
                        kind=self._policy.replacement_kind(
                            resource_signature.output_type
                        ),
                        class_type=str(node.get("class_type", "")),
                        duplicate_node_id=node_id,
                        canonical_node_id=canonical_id,
                        signature_hash=signature_hash(resource_signature.value),
                    )
                )
                if not self._context.has_remaining_references(node_id):
                    upstream = tuple(
                        source_id
                        for _, source_id, _ in self._context.linked_input_sources(
                            node_id
                        )
                    )
                    self._context.remove_node(node_id)
                    for source_id in upstream:
                        self._remove_unreferenced_resource(source_id)

    def _remove_unreferenced_resource(self, node_id: str) -> None:
        """Remove safe unreferenced Cube-owned ancestors recursively."""

        if (
            self._context.node(node_id) is None
            or self._context.is_protected(node_id)
            or self._context.has_remaining_references(node_id)
            or not self._policy.can_remove_unreferenced_node(self._context, node_id)
        ):
            return
        upstream = tuple(
            source_id for _, source_id, _ in self._context.linked_input_sources(node_id)
        )
        self._context.remove_node(node_id)
        for source_id in upstream:
            self._remove_unreferenced_resource(source_id)

    def _resolve_string(self, value: object, visiting: set[str]) -> str | None:
        """Resolve a supported literal or linked string value."""

        if isinstance(value, str):
            return value
        address = prompt_link_address(value)
        if address is None:
            return None
        source_id, output_slot = address
        if output_slot != 0 or source_id in visiting:
            return None
        source = self._context.node(source_id)
        if source is None:
            return None
        return evaluate_string_output(
            source,
            lambda nested_id, nested_slot: self._resolve_string(
                [nested_id, nested_slot], visiting | {source_id}
            ),
        )

    def _lora_references_are_scoped(self, node_id: str) -> bool:
        """Return whether all LoRA consumers are same-component supported slots."""

        for consumer_id, node in self._context.prompt.items():
            for value in node_inputs(node).values():
                address = prompt_link_address(value)
                if address is None or address[0] != node_id:
                    continue
                if address[1] not in {0, 1} or not self._context.may_share(
                    node_id, consumer_id
                ):
                    return False
        return True

    def _lora_inputs_are_scoped(
        self, node_id: str, model_input: object, clip_input: object, text_input: object
    ) -> bool:
        """Return whether all linked LoRA dependencies stay in one Cube component."""

        for value in (model_input, clip_input, text_input):
            address = prompt_link_address(value)
            if address is not None and not self._context.may_share(node_id, address[0]):
                return False
        return True


def _passthrough_label(model_input: object, clip_input: object) -> str:
    """Return a compact report target for one model/CLIP passthrough."""

    model_address = prompt_link_address(model_input)
    clip_address = prompt_link_address(clip_input)
    if (
        model_address is not None
        and clip_address is not None
        and model_address[0] == clip_address[0]
    ):
        return model_address[0]
    return "<passthrough>"
