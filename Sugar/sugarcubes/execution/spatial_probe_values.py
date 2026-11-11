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
"""Define immutable spatial-probe values independently of Comfy and canvas hosts."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Literal

from .models import ApiPrompt

SpatialKind = Literal["IMAGE", "LATENT"]


@dataclass(frozen=True)
class SpatialNodeContract:
    """Describe only shape-relevant registered node capabilities."""

    inputs: Mapping[str, str]
    outputs: tuple[str, ...]
    output_node: bool = False
    identity_input: str | None = None
    fixed_image_frame: bool = False
    dimension_limits: Mapping[str, tuple[int, int, int]] | None = None


@dataclass(frozen=True)
class SpatialFieldTarget:
    """Identify one native field's compiled prompt input without label inference."""

    node_id: str
    input_name: str


@dataclass(frozen=True)
class SpatialSource:
    """Retain the two editable inputs establishing one spatial origin."""

    node_id: str
    width: str
    height: str


@dataclass(frozen=True)
class SpatialProbeOutput:
    """Address one measurable output path explicitly selected by the user."""

    id: str
    label: str
    node_id: str
    slot: int
    kind: SpatialKind


@dataclass(frozen=True)
class SpatialProbePlan:
    """Own a detached authored prompt and its admissible spatial paths."""

    prompt: ApiPrompt
    sources: tuple[SpatialSource, ...]
    outputs: tuple[SpatialProbeOutput, ...]


@dataclass(frozen=True)
class SpatialProbeExecution:
    """Describe one bounded two-frame experiment with no ordinary image output sinks."""

    prompt: ApiPrompt
    measurement_node: str
    input_sizes: tuple[tuple[int, int], tuple[int, int]]


@dataclass(frozen=True)
class SpatialProbeMeasurement:
    """Retain the measured scale and the observations supporting it."""

    scale: float
    input_sizes: tuple[tuple[int, int], tuple[int, int]]
    output_sizes: tuple[tuple[int, int], tuple[int, int]]
