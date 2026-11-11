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
"""Dispatch approved model observations to virtual tensor container readers."""

from __future__ import annotations

from pathlib import Path

from ..application.ports import ModelRoots
from ..domain.architecture import ModelArchitecture
from ..domain.models import ModelEvidence
from .architecture_tensors import ShapeArchitectureDetector
from .safetensors_architecture import read_safetensors_architecture
from .torch_architecture_inputs import read_torch_architecture


class ApprovedModelArchitecture:
    """Enforce path approval and reject observations changed during inspection."""

    def __init__(self, roots: ModelRoots, detector: ShapeArchitectureDetector) -> None:
        """Bind filesystem approval to the externally owned detector policy."""
        self._roots = roots
        self._detector = detector

    @property
    def signature(self) -> str:
        """Share the host fingerprint with the independent cache owner."""
        return self._detector.signature

    def inspect(self, evidence: ModelEvidence) -> ModelArchitecture | None:
        """Read virtual tensor evidence only from an unchanged approved model."""
        resolved = self._roots.resolve_file(evidence.kind, evidence.value)
        if resolved is None:
            raise ValueError(
                "The model is no longer available for architecture inspection."
            )
        path = Path(resolved)
        before = path.stat()
        if (before.st_size, before.st_mtime_ns) != (
            evidence.size_bytes,
            evidence.modified_ns,
        ):
            raise ValueError("The model changed before architecture inspection.")
        extension = path.suffix.casefold()
        if extension in (".safetensors", ".sft"):
            inputs = read_safetensors_architecture(path, before.st_size)
        elif extension in (".ckpt", ".pt", ".pt2", ".bin", ".pth", ".pkl"):
            inputs = read_torch_architecture(path)
        else:
            raise ValueError("Unsupported model container for architecture inspection.")
        architecture = self._detector.detect(inputs.tensors, inputs.metadata)
        after = path.stat()
        if (after.st_size, after.st_mtime_ns, after.st_ino) != (
            before.st_size,
            before.st_mtime_ns,
            before.st_ino,
        ):
            raise ValueError("The model changed during architecture inspection.")
        return architecture
