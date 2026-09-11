#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Define the canonical workflow normalization collaboration boundary."""

from __future__ import annotations

from typing import Protocol


class WorkflowNormalizer(Protocol):
    """Normalize a serialized workflow without discarding source graph data."""

    def normalize(self, workflow_value: object) -> dict[str, object]:
        """Return a detached execution- and editing-complete workflow."""


__all__ = ["WorkflowNormalizer"]
