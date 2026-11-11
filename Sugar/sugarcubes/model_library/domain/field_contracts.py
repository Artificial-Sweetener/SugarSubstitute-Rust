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
"""Declare model field identities once for desktop and browser consumers."""

from __future__ import annotations

_EXACT_FIELDS: tuple[tuple[str, str, str], ...] = (
    ("CheckpointLoaderSimple", "ckpt_name", "checkpoints"),
    ("LoraLoader", "lora_name", "loras"),
    ("LoraLoaderModelOnly", "lora_name", "loras"),
    ("VAELoader", "vae_name", "vae"),
    ("UNETLoader", "unet_name", "diffusion_models"),
    ("UpscaleModelLoader", "model_name", "upscale_models"),
    ("ControlNetLoader", "control_net_name", "controlnet"),
    ("CLIPLoader", "clip_name", "text_encoders"),
    ("DualCLIPLoader", "clip_name1", "text_encoders"),
    ("DualCLIPLoader", "clip_name2", "text_encoders"),
    ("Power Lora Loader (rgthree)", "lora", "loras"),
    ("SimpleSyrup.LoadUltralyticsModel", "model_name", "ultralytics"),
    ("UltralyticsDetectorProvider", "model_name", "ultralytics"),
)
_FIELD_NAMES: tuple[tuple[str, str], ...] = (
    ("ckpt_name", "checkpoints"),
    ("checkpoint", "checkpoints"),
    ("checkpoint_name", "checkpoints"),
    ("lora_name", "loras"),
    ("vae_name", "vae"),
    ("unet_name", "diffusion_models"),
    ("diffusion_model", "diffusion_models"),
    ("control_net_name", "controlnet"),
    ("clip_name", "text_encoders"),
    ("clip_name1", "text_encoders"),
    ("clip_name2", "text_encoders"),
)


def model_field_contracts() -> list[dict[str, str]]:
    """Publish portable declarations without duplicating policy in picker clients."""
    return [
        {"classType": class_type, "inputName": name, "kind": kind}
        for class_type, name, kind in _EXACT_FIELDS
    ] + [
        {"classType": "", "inputName": name, "kind": kind}
        for name, kind in _FIELD_NAMES
    ]
