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
"""Provide observed model-root boundaries for isolated catalog and HTTP proofs."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

from sugarcubes.model_library.domain.models import ModelEvidence


class ObservedModelRoots:
    """Observe expensive scan requests independently from catalog reads."""

    def __init__(self) -> None:
        """Provide exact host evidence without reading model weights."""
        self.scans = 0
        self.evidence = ModelEvidence(
            "loras",
            "Anima\\alpha.safetensors",
            "primary",
            "Anima/alpha.safetensors",
            48,
            900,
        )

    def list_files(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelEvidence, ...]:
        """Record filesystem access and return deterministic evidence."""
        self.scans += 1
        return (self.evidence,) if kinds is None or self.evidence.kind in kinds else ()

    def resolve_file(self, kind: str, value: str) -> str | None:
        """Keep path resolution outside the picker catalog contract."""
        return None

    def storage_root(self) -> str:
        """Declare a root that is not accessed by the application service."""
        return "unused"

    def install_root(self, kind: str) -> str:
        """Keep observed catalog boundaries away from acquisition files."""
        raise ValueError("Observed roots cannot install files.")

    def configured_folders(self) -> dict[str, tuple[str, ...]]:
        """Keep non-filesystem observation fixtures outside periodic scanning."""
        return {}


class FixtureFileRoots:
    """Resolve only small model fixtures beneath an isolated temporary root."""

    def __init__(self, root: Path, kind: str = "loras") -> None:
        """Keep production model folders outside fixture discovery."""
        self._root = root
        self._kind = kind

    def list_files(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelEvidence, ...]:
        """Return stat observations for fixture-only model files."""
        if kinds is not None and self._kind not in kinds:
            return ()
        result: list[ModelEvidence] = []
        for path in sorted(self._root.rglob("*")):
            if not path.is_file() or path.suffix.casefold() not in (
                ".safetensors",
                ".pth",
                ".pt",
            ):
                continue
            stat = path.stat()
            result.append(
                ModelEvidence(
                    self._kind,
                    str(path.relative_to(self._root)),
                    "fixture",
                    path.relative_to(self._root).as_posix(),
                    stat.st_size,
                    stat.st_mtime_ns,
                )
            )
        return tuple(result)

    def resolve_file(self, kind: str, value: str) -> str | None:
        """Reject values that escape the isolated fixture root."""
        path = (self._root / value).resolve()
        return (
            str(path)
            if kind == self._kind
            and path.is_relative_to(self._root.resolve())
            and path.is_file()
            else None
        )

    def storage_root(self) -> str:
        """Place fixture metadata beneath the temporary model folder."""
        return str(self._root)

    def configured_folders(self) -> dict[str, tuple[str, ...]]:
        """Watch only the small isolated fixture directory."""
        return {self._kind: (str(self._root),)}

    def install_root(self, kind: str) -> str:
        """Install only into the isolated fixture category."""
        if kind != self._kind:
            raise ValueError("Unsupported fixture model category.")
        return str(self._root)
