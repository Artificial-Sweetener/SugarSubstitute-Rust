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

from ..domain.architecture import DamagedModelFile
from .architecture_tensors import ArchitectureInputs, TensorDescriptor, tensor_shape

_HEADER_LIMIT = 16 * 1024 * 1024


def read_safetensors_architecture(path: Path, file_size: int) -> ArchitectureInputs:
    """
    Read only the header and validate tensor offsets against the file observation.

    Raises `DamagedModelFile` when the file's own content cannot be a safetensors model.
    """
    with path.open("rb") as stream:
        size = stream.read(8)
        if not size:
            raise DamagedModelFile("the file is empty")
        if len(size) != 8:
            raise DamagedModelFile("the header is incomplete")
        length = struct.unpack("<Q", size)[0]
        if not 2 <= length <= _HEADER_LIMIT:
            raise DamagedModelFile("the header size is invalid")
        content = stream.read(length)
        if len(content) != length or length > file_size - 8:
            raise DamagedModelFile("the header is incomplete")
    try:
        payload: object = json.loads(content)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise DamagedModelFile("the header is not valid JSON") from error
    if not isinstance(payload, dict):
        raise DamagedModelFile("the header is not a JSON object")
    tensors: dict[str, TensorDescriptor] = {}
    metadata: dict[str, str] = {}
    for key, value in payload.items():
        if key == "__metadata__":
            if not isinstance(value, dict) or any(
                not isinstance(name, str) or not isinstance(text, str)
                for name, text in value.items()
            ):
                raise DamagedModelFile("the header metadata is invalid")
            # Comfy gates configuration overrides on recognized tensor structure.
            if isinstance(value.get("config"), str):
                metadata["config"] = value["config"]
            continue
        if not isinstance(key, str) or not key or not isinstance(value, dict):
            raise DamagedModelFile("a tensor entry in the header is invalid")
        offsets: object = value.get("data_offsets")
        if (
            not isinstance(offsets, list)
            or len(offsets) != 2
            or any(type(offset) is not int for offset in offsets)
            or not 0 <= offsets[0] <= offsets[1] <= file_size - 8 - length
        ):
            raise DamagedModelFile("a tensor's data lies outside the file")
        if not isinstance(value.get("dtype"), str):
            raise DamagedModelFile("a tensor's data type is invalid")
        tensors[key] = TensorDescriptor(tensor_shape(value.get("shape")))
    return ArchitectureInputs(tensors, metadata)
