#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Classify safe resource and value surfaces for Cube-scoped optimization."""

from __future__ import annotations

from enum import Enum

from .optimization_context import (
    PromptOptimizationContext,
    literal_inputs,
    node_options,
)


class PromptTypeClass(Enum):
    """Classify normalized Comfy values by optimization safety."""

    RESOURCE = "resource"
    PURE_VALUE = "pure_value"
    WORK = "work"
    UNKNOWN = "unknown"


class ResourceOptimizationPolicy:
    """Own the conservative resource stream rewrite policy."""

    _RESOURCE_TYPES = frozenset(
        {"MODEL", "CLIP", "VAE", "CONDITIONING", "CONDITIONING_BATCH", "HOOKS"}
    )
    _PURE_VALUE_TYPES = frozenset(
        {
            "STRING",
            "INT",
            "INTEGER",
            "FLOAT",
            "NUMBER",
            "BOOLEAN",
            "BOOL",
            "COMBO",
            "CHOICE",
        }
    )
    _WORK_TYPES = frozenset(
        {"IMAGE", "LATENT", "MASK", "AUDIO", "VIDEO", "VIDEO_FRAMES", "PREVIEW", "UI"}
    )

    def output_type_class(self, output_type: str | None) -> PromptTypeClass:
        """Classify one possibly-union Comfy type."""

        tokens = normalized_type_tokens(output_type)
        if not tokens:
            return PromptTypeClass.UNKNOWN
        if tokens & self._WORK_TYPES:
            return PromptTypeClass.WORK
        if tokens <= self._RESOURCE_TYPES:
            return PromptTypeClass.RESOURCE
        if tokens <= self._PURE_VALUE_TYPES:
            return PromptTypeClass.PURE_VALUE
        return PromptTypeClass.UNKNOWN

    def replacement_kind(self, output_type: str | None) -> str:
        """Return the stable report kind for one resource output."""

        tokens = normalized_type_tokens(output_type)
        if tokens == {"MODEL"}:
            return "model_resource_stream"
        if tokens == {"CLIP"}:
            return "clip_resource_stream"
        if tokens == {"VAE"}:
            return "vae_resource_stream"
        if tokens <= {"CONDITIONING", "CONDITIONING_BATCH"}:
            return "conditioning_resource_stream"
        return "resource_stream"

    def can_sign_output(
        self, context: PromptOptimizationContext, node_id: str, output_slot: int
    ) -> bool:
        """Return whether an output has a safe recursive signature surface."""

        if context.is_loose(node_id) or not self._has_safe_surface(context, node_id):
            return False
        output_class = self.output_type_class(context.output_type(node_id, output_slot))
        if output_class not in {PromptTypeClass.RESOURCE, PromptTypeClass.PURE_VALUE}:
            return False
        return all(
            self.output_type_class(context.output_type(source_id, source_slot))
            in {PromptTypeClass.RESOURCE, PromptTypeClass.PURE_VALUE}
            for _, source_id, source_slot in context.linked_input_sources(node_id)
        )

    def root_identity_is_visible(
        self, context: PromptOptimizationContext, node_id: str
    ) -> bool:
        """Return whether a root exposes literals or options that identify it."""

        node = context.node(node_id)
        if node is None:
            return False
        inputs = context.inputs_for_node(node_id)
        return bool(literal_inputs(inputs)) or node_options(node) != ()

    def intern_output_decision(
        self, context: PromptOptimizationContext, node_id: str, output_slot: int
    ) -> tuple[bool, str]:
        """Return resource rewrite eligibility and a stable diagnostic reason."""

        if context.is_loose(node_id):
            return False, "loose_owner"
        definition = context.definition_for_node(node_id)
        if definition is None:
            return False, "missing_definition"
        if definition.output_node:
            return False, "output_node"
        if definition.hidden_inputs:
            return False, "hidden_inputs"
        output_class = self.output_type_class(context.output_type(node_id, output_slot))
        if output_class is PromptTypeClass.WORK:
            return False, "work_output"
        if output_class is PromptTypeClass.UNKNOWN:
            return False, "unknown_output_type"
        if output_class is PromptTypeClass.PURE_VALUE:
            return False, "pure_value_not_rewrite_target"
        links = context.linked_input_sources(node_id)
        if not links:
            return False, "resource_root_not_rewrite_target"
        for _, source_id, source_slot in links:
            source_class = self.output_type_class(
                context.output_type(source_id, source_slot)
            )
            if source_class is PromptTypeClass.WORK:
                return False, "linked_work_input"
            if source_class is PromptTypeClass.UNKNOWN:
                return False, "linked_unknown_input"
        return True, "eligible"

    def can_remove_unreferenced_node(
        self, context: PromptOptimizationContext, node_id: str
    ) -> bool:
        """Return whether a Cube-owned unreferenced resource ancestor is removable."""

        if context.is_loose(node_id) or not self._has_safe_surface(context, node_id):
            return False
        return all(
            self.output_type_class(context.output_type(source_id, output_slot))
            in {PromptTypeClass.RESOURCE, PromptTypeClass.PURE_VALUE}
            for _, source_id, output_slot in context.linked_input_sources(node_id)
        )

    def _has_safe_surface(
        self, context: PromptOptimizationContext, node_id: str
    ) -> bool:
        """Return whether every output is a pure resource or value surface."""

        definition = context.definition_for_node(node_id)
        return (
            definition is not None
            and not definition.output_node
            and not definition.hidden_inputs
            and bool(definition.output_types)
            and all(
                self.output_type_class(output_type)
                in {PromptTypeClass.RESOURCE, PromptTypeClass.PURE_VALUE}
                for output_type in definition.output_types
            )
        )


def normalized_type_tokens(type_name: str | None) -> frozenset[str]:
    """Normalize a comma-separated Comfy type union."""

    if type_name is None:
        return frozenset()
    return frozenset(
        token.strip().upper() for token in type_name.split(",") if token.strip()
    )
