#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Define host-independent ports for final Comfy validation and queueing."""

from __future__ import annotations

from typing import Protocol

from .models import ComfyQueueReceipt, PreparedCubeExecution


class ComfyExecutionPort(Protocol):
    """Validate and atomically queue one fully prepared SugarCubes prompt."""

    async def queue(self, prepared: PreparedCubeExecution) -> ComfyQueueReceipt:
        """Return Comfy's normalized acceptance or validation rejection."""
