#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Define public failures for canonical Cube graph mutations."""


class CubeGraphMutationError(ValueError):
    """Reject a graph mutation that would cross or reinterpret a graph boundary."""


__all__ = ["CubeGraphMutationError"]
