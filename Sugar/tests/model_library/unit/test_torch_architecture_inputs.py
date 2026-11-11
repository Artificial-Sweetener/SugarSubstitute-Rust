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
"""Prove checkpoint shape inspection uses restricted deserialization and virtual storage."""

from __future__ import annotations

from pathlib import Path
import pickle
import sys
from types import ModuleType, TracebackType

import pytest

from sugarcubes.model_library.infrastructure.architecture_tensors import (
    TensorDescriptor,
)
from sugarcubes.model_library.infrastructure.torch_architecture_inputs import (
    read_torch_architecture,
)


@pytest.mark.parametrize("wrapper", ["direct", "state_dict", "single"])
def test_checkpoint_wrapper_inspection_never_loads_real_storage(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, wrapper: str
) -> None:
    """Follow Comfy's checkpoint wrappers while enforcing safe virtual deserialization."""
    active: list[bool] = []

    class VirtualMode:
        """Observe the virtual storage context around the external loader."""

        def __enter__(self) -> VirtualMode:
            """Mark deserialization as virtual before any model is read."""
            active.append(True)
            return self

        def __exit__(
            self,
            exc_type: type[BaseException] | None,
            error: BaseException | None,
            traceback: TracebackType | None,
        ) -> None:
            """Release the virtual loader context even on failure."""
            active.pop()

    tensors = {"weight": TensorDescriptor((320, 4, 3, 3)), "training_step": 10}
    payload: object = (
        tensors
        if wrapper == "direct"
        else {"state_dict" if wrapper == "state_dict" else "model": tensors}
    )

    def load(path: Path, *, map_location: str, weights_only: bool) -> object:
        """Reject loader calls that could allocate weights or execute arbitrary pickles."""
        assert active == [True] and weights_only is True and map_location == "cpu"
        return payload

    torch = ModuleType("torch")
    virtual = ModuleType("torch._subclasses.fake_tensor")
    setattr(torch, "Tensor", TensorDescriptor)
    setattr(torch, "load", load)
    setattr(virtual, "FakeTensorMode", VirtualMode)
    monkeypatch.setitem(sys.modules, "torch", torch)
    monkeypatch.setitem(sys.modules, "torch._subclasses.fake_tensor", virtual)
    assert read_torch_architecture(tmp_path / "model.ckpt").tensors == {
        "weight": TensorDescriptor((320, 4, 3, 3))
    }
    assert active == []


def test_unsafe_checkpoint_is_rejected_without_unrestricted_retry(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Preserve the restricted loader boundary when a checkpoint requires arbitrary globals."""
    from contextlib import nullcontext

    calls: list[bool] = []

    def load(path: Path, *, map_location: str, weights_only: bool) -> object:
        """Simulate PyTorch refusing an unsafe pickle global."""
        calls.append(weights_only)
        raise pickle.UnpicklingError("Unsupported global")

    torch = ModuleType("torch")
    virtual = ModuleType("torch._subclasses.fake_tensor")
    setattr(torch, "load", load)
    setattr(virtual, "FakeTensorMode", nullcontext)
    monkeypatch.setitem(sys.modules, "torch", torch)
    monkeypatch.setitem(sys.modules, "torch._subclasses.fake_tensor", virtual)
    with pytest.raises(ValueError, match="restricted virtual loader"):
        read_torch_architecture(tmp_path / "unsafe.pkl")
    assert calls == [True]
