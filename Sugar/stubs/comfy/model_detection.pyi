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
"""Describe the Comfy shape-only configuration detection boundary."""

from collections.abc import Mapping
from typing import Protocol, TypeVar

class TensorShape(Protocol):
    """Expose dimensions consumed by header-only model detection."""

    @property
    def shape(self) -> tuple[int, ...]: ...
    @property
    def ndim(self) -> int: ...
    def size(self, dim: int) -> int: ...

def unet_prefix_from_state_dict(state_dict: Mapping[str, TensorShape]) -> str: ...
def detect_unet_config(
    state_dict: Mapping[str, TensorShape], key_prefix: str, metadata: object = ...
) -> dict[str, object] | None: ...
def model_config_from_unet_config(
    unet_config: dict[str, object],
    state_dict: Mapping[str, TensorShape] | None = ...,
    unet_key_prefix: str = ...,
) -> object | None: ...
def model_config_from_diffusers_unet(
    state_dict: Mapping[str, TensorShape],
) -> object | None: ...

_Tensor = TypeVar("_Tensor", bound=TensorShape)

def convert_diffusers_mmdit(
    state_dict: dict[str, _Tensor],
    output_prefix: str = ...,
) -> dict[str, _Tensor] | None: ...
