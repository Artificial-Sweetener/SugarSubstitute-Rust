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

from enum import Enum


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


def classify_input_persistence(
    class_type: str,
    input_name: str,
) -> InputPersistence:
    """Return the canonical persistence disposition for one node input."""

    normalized_class = class_type.strip()
    normalized_name = input_name.strip()
    if normalized_name == "seed":
        return InputPersistence.VOLATILE
    if normalized_name in _LOCAL_RESOURCE_INPUT_NAMES:
        return InputPersistence.LOCAL_RESOURCE
    if (normalized_class, normalized_name) in _LOCAL_RESOURCE_FIELDS:
        return InputPersistence.LOCAL_RESOURCE
    return InputPersistence.PORTABLE_AUTHORED


def should_store_authored_value(class_type: str, input_name: str) -> bool:
    """Return whether a cube may ship an authored value for one input."""

    return (
        classify_input_persistence(class_type, input_name)
        is InputPersistence.PORTABLE_AUTHORED
    )
