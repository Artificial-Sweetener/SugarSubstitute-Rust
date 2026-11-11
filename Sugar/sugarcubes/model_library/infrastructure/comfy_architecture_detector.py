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
"""Delegate tensor architecture policy to the installed Comfy model detector."""

from __future__ import annotations

from collections.abc import Mapping
import hashlib
from pathlib import Path

from ..domain.architecture import ModelArchitecture
from .architecture_tensors import TensorDescriptor


class ComfyArchitectureDetector:
    """Classify shape descriptors without calling checkpoint loaders or model allocation."""

    def __init__(self) -> None:
        """Defer host imports until architecture inspection is requested."""
        self._signature: str | None = None

    @property
    def signature(self) -> str:
        """Invalidate persisted results when either host detection or model matching changes."""
        if self._signature is None:
            from comfy import (
                model_detection,
                supported_models,
                supported_models_base,
                utils,
            )

            digest = hashlib.sha256(b"sugarcubes-comfy-virtual-tensors-v2")
            for module in (
                model_detection,
                supported_models,
                supported_models_base,
                utils,
            ):
                if module.__file__ is None:
                    raise ValueError("Comfy architecture source is unavailable.")
                digest.update(Path(module.__file__).read_bytes())
            self._signature = digest.hexdigest()
        return self._signature

    def detect(
        self,
        tensors: Mapping[str, TensorDescriptor],
        metadata: Mapping[str, str] | None = None,
    ) -> ModelArchitecture | None:
        """Follow core Comfy's native, converted Diffusers, and Diffusers UNet paths."""
        from comfy import model_detection

        prefix = model_detection.unet_prefix_from_state_dict(tensors)
        if not any(key.startswith(prefix) for key in tensors):
            prefix = ""
        config = model_detection.detect_unet_config(tensors, prefix, metadata=metadata)
        model = (
            model_detection.model_config_from_unet_config(config, tensors, prefix)
            if config is not None
            else None
        )
        if model is None:
            import torch

            virtual = {
                key[len(prefix) :]: torch.empty(tensor.shape, device="meta")
                for key, tensor in tensors.items()
                if key.startswith(prefix)
            }
            converted = model_detection.convert_diffusers_mmdit(virtual, "")
            if converted is not None:
                config = model_detection.detect_unet_config(
                    converted, "", metadata=metadata
                )
                model = (
                    model_detection.model_config_from_unet_config(config, converted, "")
                    if config is not None
                    else None
                )
            else:
                model = model_detection.model_config_from_diffusers_unet(virtual)
        if model is None:
            return None
        model_class = type(model)
        bases = {base.__name__ for base in model_class.__mro__}
        families = (
            ("SDXL", "SDXL"),
            ("SDXLRefiner", "SDXL"),
            ("SD15", "SD1.5"),
            ("SD20", "SD2"),
            ("Flux2", "FLUX.2"),
            ("Flux", "FLUX"),
        )
        base = next(
            (label for name, label in families if name in bases), model_class.__name__
        )
        return ModelArchitecture(base, model_class.__name__)
