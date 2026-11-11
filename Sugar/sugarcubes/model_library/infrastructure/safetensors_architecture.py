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
"""Read bounded safetensors headers without consulting declared model labels."""

from __future__ import annotations

import json
from pathlib import Path
import struct

from .architecture_tensors import ArchitectureInputs, TensorDescriptor, tensor_shape

_HEADER_LIMIT = 16 * 1024 * 1024


def read_safetensors_architecture(path: Path, file_size: int) -> ArchitectureInputs:
    """Read only the header and validate tensor offsets against the file observation."""
    with path.open("rb") as stream:
        size = stream.read(8)
        if len(size) != 8:
            raise ValueError("The safetensors header length is missing.")
        length = struct.unpack("<Q", size)[0]
        if not 2 <= length <= _HEADER_LIMIT or length > file_size - 8:
            raise ValueError("The safetensors header size is invalid.")
        content = stream.read(length)
        if len(content) != length:
            raise ValueError("The safetensors header is truncated.")
    payload: object = json.loads(content)
    if not isinstance(payload, dict):
        raise ValueError("The safetensors header must be an object.")
    tensors: dict[str, TensorDescriptor] = {}
    metadata: dict[str, str] = {}
    for key, value in payload.items():
        if key == "__metadata__":
            if not isinstance(value, dict) or any(
                not isinstance(name, str) or not isinstance(text, str)
                for name, text in value.items()
            ):
                raise ValueError("Invalid safetensors metadata.")
            # Comfy gates configuration overrides on recognized tensor structure.
            if isinstance(value.get("config"), str):
                metadata["config"] = value["config"]
            continue
        if not isinstance(key, str) or not key or not isinstance(value, dict):
            raise ValueError("Invalid tensor header entry.")
        offsets: object = value.get("data_offsets")
        if (
            not isinstance(offsets, list)
            or len(offsets) != 2
            or any(type(offset) is not int for offset in offsets)
            or not 0 <= offsets[0] <= offsets[1] <= file_size - 8 - length
        ):
            raise ValueError("Invalid tensor data offsets.")
        if not isinstance(value.get("dtype"), str):
            raise ValueError("Invalid tensor dtype.")
        tensors[key] = TensorDescriptor(tensor_shape(value.get("shape")))
    return ArchitectureInputs(tensors, metadata)
