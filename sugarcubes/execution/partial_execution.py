#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Resolve stable Cube partial targets to prompt-only output sink ids."""

from __future__ import annotations

from dataclasses import replace

from .errors import CubeQueueError
from .models import CubeOutputIdentity, CubeQueueOptions


def resolve_partial_execution_targets(
    queue: CubeQueueOptions,
    identities: tuple[CubeOutputIdentity, ...],
) -> CubeQueueOptions:
    """Translate instance, boundary, or sink targets without guessing."""

    if not queue.partial_execution_targets:
        return queue
    resolved: list[str] = []
    for target in queue.partial_execution_targets:
        matches = tuple(
            identity.execution_id
            for identity in identities
            if target
            in {
                identity.execution_id,
                identity.instance_id,
                f"{identity.instance_id}:{identity.binding}",
            }
        )
        if not matches:
            raise CubeQueueError(
                "execution.queue.unknown_partial_target",
                f"Partial execution target '{target}' does not identify a Cube output.",
            )
        for execution_id in matches:
            if execution_id not in resolved:
                resolved.append(execution_id)
    return replace(queue, partial_execution_targets=tuple(resolved))
