#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Provide a bounded CPU-only Comfy node for observing generation progress."""

from __future__ import annotations

from importlib import import_module
from time import sleep
from typing import Callable, ClassVar, Protocol, cast


class _ProgressPort(Protocol):
    """Describe the public Comfy progress operation used by the browser proof."""

    def update_absolute(self, value: int, total: int) -> None:
        """Publish one real backend sampler step."""


class GenerationProgressProof:
    """Run short deterministic steps without model files or GPU allocation."""

    RETURN_TYPES: ClassVar[tuple[str, ...]] = ("INT",)
    FUNCTION: ClassVar[str] = "run"
    OUTPUT_NODE: ClassVar[bool] = True
    CATEGORY: ClassVar[str] = "testing/SugarCubes"

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, object]:
        """Expose bounded inputs so a two-node chain reports independent progress."""

        return {
            "required": {
                "value": ("INT", {"default": 1}),
                "steps": ("INT", {"default": 20, "min": 1, "max": 40}),
                "pause": ("FLOAT", {"default": 0.12, "min": 0, "max": 0.2}),
            }
        }

    def run(self, value: int, steps: int, pause: float) -> tuple[int]:
        """Emit native progress events while passing one integer through the graph."""

        if not 1 <= steps <= 40 or not 0 <= pause <= 0.2:
            raise ValueError("Progress proof inputs exceed the bounded test workload")
        candidate: object = getattr(import_module("comfy.utils"), "ProgressBar")
        if not callable(candidate):
            raise TypeError("Comfy's public progress API is unavailable")
        # Comfy supplies the runtime class without an installed static type contract.
        factory = cast(Callable[[int], _ProgressPort], candidate)
        progress = factory(steps)
        for step in range(steps):
            sleep(pause)
            progress.update_absolute(step + 1, steps)
        return (value + 1,)


NODE_CLASS_MAPPINGS: dict[str, type[GenerationProgressProof]] = {
    "GenerationProgressProof": GenerationProgressProof,
}
