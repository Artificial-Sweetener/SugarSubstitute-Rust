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
"""Inspect PyTorch checkpoint containers with the host's restricted virtual loader."""

from __future__ import annotations

from pathlib import Path
import pickle

from .architecture_tensors import ArchitectureInputs, TensorDescriptor, tensor_shape


def read_torch_architecture(path: Path) -> ArchitectureInputs:
    """Extract tensor shapes without weight storage reads or unrestricted unpickling."""
    import torch
    from torch._subclasses.fake_tensor import FakeTensorMode

    try:
        with FakeTensorMode():
            payload: object = torch.load(path, map_location="cpu", weights_only=True)
    except (pickle.UnpicklingError, RuntimeError, EOFError) as error:
        raise ValueError(
            "The model cannot be inspected with PyTorch's restricted virtual loader."
        ) from error
    if isinstance(payload, dict):
        if "state_dict" in payload:
            payload = payload["state_dict"]
        elif len(payload) == 1:
            nested: object = next(iter(payload.values()))
            if isinstance(nested, dict):
                payload = nested
    if not isinstance(payload, dict):
        raise ValueError("The model checkpoint must contain a tensor dictionary.")
    tensors: dict[str, TensorDescriptor] = {}
    for name, value in payload.items():
        if not isinstance(name, str) or not name:
            raise ValueError("Invalid checkpoint tensor name.")
        if isinstance(value, torch.Tensor):
            tensors[name] = TensorDescriptor(tensor_shape(tuple(value.shape)))
    return ArchitectureInputs(tensors)
