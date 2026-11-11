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
"""Prove the architecture adapter calls Comfy's detector and matcher without model loaders."""

from __future__ import annotations

from collections.abc import Mapping
import sys
from types import ModuleType

import pytest

from sugarcubes.model_library.domain.architecture import ModelArchitecture
from sugarcubes.model_library.infrastructure.comfy_architecture_detector import (
    ComfyArchitectureDetector,
)
from sugarcubes.model_library.infrastructure.architecture_tensors import (
    TensorDescriptor,
)


def test_diffusers_conversion_uses_native_detection_after_conversion(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Follow Comfy's conversion fallback before the diffusers UNet matcher."""

    class FutureComfyModel:
        """Represent a new host configuration without an extension whitelist."""

    module = ModuleType("comfy.model_detection")
    torch_module = ModuleType("torch")
    setattr(
        torch_module,
        "empty",
        lambda shape, device: (
            TensorDescriptor(tuple(shape))
            if device == "meta"
            else pytest.fail("Real tensor allocation")
        ),
    )
    calls: list[str] = []

    def detect(
        state: Mapping[str, TensorDescriptor], prefix: str, metadata: object = None
    ) -> dict[str, object] | None:
        """Recognize only the host-converted representation."""
        calls.append("detect")
        return {"converted": True} if "native.weight" in state else None

    def convert(
        state: dict[str, TensorDescriptor], prefix: str
    ) -> dict[str, TensorDescriptor]:
        """Observe a mutable virtual tensor dictionary at Comfy's conversion boundary."""
        calls.append("convert")
        assert state["diffusers.weight"].shape == (4, 8)
        return {"native.weight": state["diffusers.weight"]}

    setattr(module, "unet_prefix_from_state_dict", lambda state: "model.")
    setattr(module, "detect_unet_config", detect)
    setattr(module, "convert_diffusers_mmdit", convert)
    setattr(
        module,
        "model_config_from_unet_config",
        lambda config, state, prefix: FutureComfyModel(),
    )
    host = ModuleType("comfy")
    setattr(host, "model_detection", module)
    monkeypatch.setitem(sys.modules, "comfy", host)
    monkeypatch.setitem(sys.modules, "comfy.model_detection", module)
    monkeypatch.setitem(sys.modules, "torch", torch_module)
    assert ComfyArchitectureDetector().detect(
        {"diffusers.weight": TensorDescriptor((4, 8))}
    ) == ModelArchitecture("FutureComfyModel", "FutureComfyModel")
    assert calls == ["detect", "convert", "detect"]


def test_host_detector_owns_architecture_decision(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Forward shapes to native detection and preserve the exact returned configuration class."""

    class SDXL:
        """Stand in for the external host's configuration instance."""

    class SDXL_instructpix2pix(SDXL):
        """Preserve host-derived inheritance for broad architecture classification."""

    module = ModuleType("comfy.model_detection")
    calls: list[str] = []
    tensors = {"model.diffusion_model.weight": TensorDescriptor((320, 4, 3, 3))}

    def prefix(state: Mapping[str, TensorDescriptor]) -> str:
        """Observe native prefix selection."""
        assert state is tensors
        calls.append("prefix")
        return "model.diffusion_model."

    def detect(
        state: Mapping[str, TensorDescriptor], key_prefix: str, metadata: object = None
    ) -> dict[str, object]:
        """Receive only shape evidence with metadata omitted."""
        assert state is tensors and key_prefix == "model.diffusion_model."
        calls.append("detect")
        return {"host_decision": True}

    def match(
        config: dict[str, object],
        state: Mapping[str, TensorDescriptor],
        key_prefix: str,
    ) -> object:
        """Require the adapter to use the host matcher rather than its own shape heuristics."""
        assert config == {"host_decision": True} and state is tensors
        calls.append("match")
        return SDXL_instructpix2pix()

    setattr(module, "unet_prefix_from_state_dict", prefix)
    setattr(module, "detect_unet_config", detect)
    setattr(module, "model_config_from_unet_config", match)
    host = ModuleType("comfy")
    setattr(host, "model_detection", module)
    monkeypatch.setitem(sys.modules, "comfy", host)
    monkeypatch.setitem(sys.modules, "comfy.model_detection", module)
    assert ComfyArchitectureDetector().detect(tensors) == ModelArchitecture(
        "SDXL", "SDXL_instructpix2pix"
    )
    assert calls == ["prefix", "detect", "match"]


@pytest.mark.parametrize("recognized", [True, False])
def test_diffusers_unet_fallback_and_unknown_models(
    monkeypatch: pytest.MonkeyPatch, recognized: bool
) -> None:
    """Use the final core matcher after conversion declines without inventing unknown families."""

    class SD15:
        """Represent the host's recognized final fallback configuration."""

    module = ModuleType("comfy.model_detection")
    torch_module = ModuleType("torch")

    def empty(shape: tuple[int, ...], *, device: str) -> TensorDescriptor:
        """Permit virtual allocations only."""
        assert device == "meta"
        return TensorDescriptor(shape)

    def fallback(state: Mapping[str, TensorDescriptor]) -> object | None:
        """Observe stripped diffusion keys with original dimensions."""
        assert state == {"conv_in.weight": TensorDescriptor((320, 4, 3, 3))}
        return SD15() if recognized else None

    setattr(
        module, "unet_prefix_from_state_dict", lambda state: "model.diffusion_model."
    )
    setattr(module, "detect_unet_config", lambda state, prefix, metadata: None)
    setattr(module, "convert_diffusers_mmdit", lambda state, prefix: None)
    setattr(module, "model_config_from_diffusers_unet", fallback)
    setattr(torch_module, "empty", empty)
    host = ModuleType("comfy")
    setattr(host, "model_detection", module)
    monkeypatch.setitem(sys.modules, "comfy", host)
    monkeypatch.setitem(sys.modules, "comfy.model_detection", module)
    monkeypatch.setitem(sys.modules, "torch", torch_module)
    tensors = {
        "model.diffusion_model.conv_in.weight": TensorDescriptor((320, 4, 3, 3)),
        "unrelated.weight": TensorDescriptor((1,)),
    }
    result = ComfyArchitectureDetector().detect(tensors)
    assert result == (ModelArchitecture("SD1.5", "SD15") if recognized else None)
