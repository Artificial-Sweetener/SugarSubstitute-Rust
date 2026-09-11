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
"""Classify node inputs by their authoritative cube persistence behavior."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from enum import Enum
from typing import Any

from .picker_fields import is_picker_field_spec, picker_options


class InputPersistence(str, Enum):
    """Describe where one name-addressed node input value may persist."""

    PORTABLE_AUTHORED = "portable_authored"
    LOCAL_RESOURCE = "local_resource"
    VOLATILE = "volatile"


_LOCAL_RESOURCE_INPUT_NAMES = frozenset(
    {
        "checkpoint_name",
        "ckpt_name",
        "clip_name",
        "clip_name1",
        "clip_name2",
        "clip_name3",
        "control_net_name",
        "lora_name",
        "unet_name",
        "vae_name",
    }
)

_LOCAL_RESOURCE_FIELDS = frozenset(
    {
        ("LoadImage", "image"),
        ("LoadImageMask", "image"),
        ("SeedVR2LoadDiTModel", "device"),
        ("SeedVR2LoadDiTModel", "model"),
        ("UltralyticsDetectorProvider", "model_name"),
        ("UpscaleModelLoader", "model_name"),
    }
)

_LOCAL_FILE_METADATA_KEYS = frozenset({"image_folder"})
_LOCAL_FILE_METADATA_SUFFIXES = ("_path_extensions",)
_LOCAL_RESOURCE_NAME_SUFFIXES = (
    "_checkpoint",
    "_checkpoint_name",
    "_diffusion_model",
    "_encoder",
    "_encoder_name",
    "_model",
    "_model_name",
    "_text_encoder",
    "_unet",
    "_unet_name",
    "_vae",
    "_vae_name",
)
_LOCAL_RESOURCE_EXACT_NAMES = frozenset({"model", "text_encoder", "vae"})
_LOCAL_RESOURCE_FILE_EXTENSIONS = (
    ".bin",
    ".ckpt",
    ".engine",
    ".gguf",
    ".onnx",
    ".pt",
    ".pth",
    ".safetensors",
)


def classify_input_persistence(
    class_type: str,
    input_name: str,
    *,
    field_spec: Any = None,
) -> InputPersistence:
    """Return the canonical persistence disposition for one node input."""

    normalized_class = class_type.strip()
    normalized_name = input_name.strip()
    if normalized_name == "seed":
        return InputPersistence.VOLATILE
    if is_local_file_field_spec(field_spec):
        return InputPersistence.LOCAL_RESOURCE
    if normalized_name in _LOCAL_RESOURCE_INPUT_NAMES:
        return InputPersistence.LOCAL_RESOURCE
    if _picker_contains_local_resource_paths(field_spec):
        return InputPersistence.LOCAL_RESOURCE
    if (
        _is_resource_shaped_input_name(normalized_name)
        and is_picker_field_spec(field_spec)
        and not picker_options(field_spec)
    ):
        return InputPersistence.LOCAL_RESOURCE
    if (normalized_class, normalized_name) in _LOCAL_RESOURCE_FIELDS:
        return InputPersistence.LOCAL_RESOURCE
    return InputPersistence.PORTABLE_AUTHORED


def should_store_authored_value(
    class_type: str,
    input_name: str,
    *,
    field_spec: Any = None,
) -> bool:
    """Return whether a cube may ship an authored value for one input."""

    return (
        classify_input_persistence(
            class_type,
            input_name,
            field_spec=field_spec,
        )
        is InputPersistence.PORTABLE_AUTHORED
    )


def is_local_file_field_spec(field_spec: Any) -> bool:
    """Return whether Comfy metadata declares one filesystem-backed input."""

    metadata = _field_metadata(field_spec)
    for key, value in metadata.items():
        normalized_key = str(key).strip().lower()
        if normalized_key.endswith("_upload") and value is True:
            return True
        if normalized_key in _LOCAL_FILE_METADATA_KEYS:
            return True
        if normalized_key.endswith(_LOCAL_FILE_METADATA_SUFFIXES):
            return True
    return False


def _is_resource_shaped_input_name(input_name: str) -> bool:
    """Recognize conventional loader resource identities without node coupling."""

    normalized = input_name.strip().lower()
    return normalized in _LOCAL_RESOURCE_EXACT_NAMES or normalized.endswith(
        _LOCAL_RESOURCE_NAME_SUFFIXES
    )


def _picker_contains_local_resource_paths(field_spec: Any) -> bool:
    """Recognize host inventory pickers from their file-shaped options."""

    return any(
        isinstance(option, str)
        and option.strip().lower().endswith(_LOCAL_RESOURCE_FILE_EXTENSIONS)
        for option in picker_options(field_spec)
    )


def _field_metadata(field_spec: Any) -> Mapping[str, Any]:
    """Return metadata paired with one Comfy input field specification."""

    if (
        isinstance(field_spec, Sequence)
        and not isinstance(field_spec, (str, bytes))
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return {}
