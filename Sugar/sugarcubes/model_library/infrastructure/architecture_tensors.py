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
"""Define validated virtual tensor inputs shared by model-container adapters."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Protocol

from ..domain.architecture import ModelArchitecture


@dataclass(frozen=True, slots=True)
class TensorDescriptor:
    """Expose dimensions without materializing model weights."""

    shape: tuple[int, ...]

    @property
    def ndim(self) -> int:
        """Answer Comfy's rank queries without a tensor allocation."""
        return len(self.shape)

    def size(self, dim: int) -> int:
        """Answer Comfy's dimension queries without a tensor allocation."""
        return self.shape[dim]


@dataclass(frozen=True, slots=True)
class ArchitectureInputs:
    """Carry tensor evidence and gated configuration metadata to host detection."""

    tensors: Mapping[str, TensorDescriptor]
    metadata: Mapping[str, str] = field(default_factory=dict)


class ShapeArchitectureDetector(Protocol):
    """Keep container parsing independent of Comfy's architecture policy."""

    @property
    def signature(self) -> str:
        """Identify host policy changes for persistent cache invalidation."""

    def detect(
        self,
        tensors: Mapping[str, TensorDescriptor],
        metadata: Mapping[str, str] | None = None,
    ) -> ModelArchitecture | None:
        """Classify validated evidence through the host's detector."""


def tensor_shape(value: object) -> tuple[int, ...]:
    """Reject malformed dimensions before creating virtual tensors."""
    if (
        not isinstance(value, (tuple, list))
        or len(value) > 16
        or any(type(dim) is not int or dim < 0 for dim in value)
    ):
        raise ValueError("Invalid tensor dimensions.")
    return tuple(value)
