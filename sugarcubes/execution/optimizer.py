#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Run one deterministic SugarCubes-owned prompt optimization sequence."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from typing import Protocol

from .models import (
    ApiPrompt,
    CubeOptimizationOptions,
    CubeOptimizationReport,
    CubeTopology,
    ExecutionNodeOwner,
    NodeDefinitions,
)
from .optimization_context import PromptOptimizationContext
from .optimization_passes import CubeOptimizationPasses
from .optimization_policy import ResourceOptimizationPolicy

_logger = logging.getLogger(__name__)


class PromptOptimizer(Protocol):
    """Define the optimizer boundary used by the execution coordinator."""

    def optimize(
        self,
        prompt: ApiPrompt,
        *,
        node_owners: Mapping[str, ExecutionNodeOwner],
        node_definitions: NodeDefinitions,
        topology: CubeTopology,
        options: CubeOptimizationOptions,
        protected_node_ids: frozenset[str] = frozenset(),
    ) -> tuple[ApiPrompt, CubeOptimizationReport]:
        """Return a derived prompt and deterministic optimization report."""


class CubePromptOptimizer:
    """Optimize only nodes sharing one non-loose Cube topology component."""

    def __init__(self, policy: ResourceOptimizationPolicy | None = None) -> None:
        """Bind the conservative policy used by all optimizer passes."""

        self._policy = policy or ResourceOptimizationPolicy()

    def optimize(
        self,
        prompt: ApiPrompt,
        *,
        node_owners: Mapping[str, ExecutionNodeOwner],
        node_definitions: NodeDefinitions,
        topology: CubeTopology,
        options: CubeOptimizationOptions,
        protected_node_ids: frozenset[str] = frozenset(),
    ) -> tuple[ApiPrompt, CubeOptimizationReport]:
        """Return a copied optimized prompt and a deterministic rewrite report."""

        context = PromptOptimizationContext(
            prompt,
            node_owners=node_owners,
            node_definitions=node_definitions,
            component_by_instance=topology.component_index(),
            protected_node_ids=protected_node_ids,
        )
        if not options.enabled:
            return context.prompt, CubeOptimizationReport.unchanged(len(prompt))
        replacements, pass_counts = CubeOptimizationPasses(context, self._policy).run(
            options
        )
        report = CubeOptimizationReport(
            optimized=bool(replacements),
            original_node_count=len(prompt),
            optimized_node_count=len(context.prompt),
            replacements=tuple(replacements),
            pass_counts=pass_counts,
        )
        _logger.debug(
            "Optimized SugarCubes prompt original_nodes=%d optimized_nodes=%d replacements=%d",
            report.original_node_count,
            report.optimized_node_count,
            len(report.replacements),
        )
        return context.prompt, report
