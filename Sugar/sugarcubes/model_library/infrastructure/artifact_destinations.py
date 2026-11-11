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
"""Own approved destinations and cross-process reservations for non-overwriting publication."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import secrets
import tempfile
from urllib.parse import urlsplit

from ..domain.artifact_safety import safe_artifact_name
from ..domain.download_paths import valid_download_relative_path
from ..domain.provider_releases import ModelArtifactOffer


class ModelArtifactDestination:
    """Reserve reference-named slots without exposing temporary bytes as Comfy model choices."""

    def __init__(
        self, root: str, offer: ModelArtifactOffer, relative: str | None
    ) -> None:
        """Bind one approved folder and retain the provider's collision version identity."""
        self._root = Path(root).resolve()
        name = relative if relative is not None else offer.file_name
        if not valid_download_relative_path(name) or not safe_artifact_name(
            name.rsplit("/", 1)[-1]
        ):
            raise ValueError("Invalid model artifact destination.")
        self._target = self._root / name
        self._original = self._target
        self._version = (
            int(offer.sha256[:8], 16)
            if offer.provider == "openmodeldb"
            else int(urlsplit(offer.download_url).path.rsplit("/", 1)[-1])
        )
        self._marker: Path | None = None
        self._token = secrets.token_hex(16).encode("ascii")
        self._committed = False
        self.validate()

    @property
    def folder(self) -> Path:
        """Expose the validated transaction folder for hashing and temporary storage."""
        return self._original.parent

    @property
    def committed_name(self) -> str | None:
        """Retain the filesystem commit point even if later cancellation interrupts cleanup."""
        return str(self._target.relative_to(self._root)) if self._committed else None

    def reuse(self, path: Path) -> str:
        """Commit an already verified file only inside the reviewed destination folder."""
        if path.parent != self.folder or path.is_symlink():
            raise ValueError("Invalid installed model destination.")
        self._target = path
        self.validate()
        self._committed = True
        return str(path.relative_to(self._root))

    def reserve(self) -> None:
        """Claim original then version-suffixed slots atomically across independent processes."""
        self.validate()
        self.folder.mkdir(parents=True, exist_ok=True)
        self.validate()
        descriptor, name = tempfile.mkstemp(
            prefix=".sugarcubes-", suffix=".reservation", dir=self.folder
        )
        token_file = Path(name)
        try:
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(self._token)
                stream.flush()
                os.fsync(stream.fileno())
            for index in range(-1, 1000):
                candidate = (
                    self._original
                    if index == -1
                    else self._original.with_name(
                        f"{self._original.stem} (v{self._version}{'' if index == 0 else f'-{index}'}){self._original.suffix}"
                    )
                )
                self._target = candidate
                if not valid_download_relative_path(
                    candidate.relative_to(self._root).as_posix()
                ):
                    raise ValueError("Invalid model artifact destination.")
                self.validate()
                if candidate.exists() or candidate.is_symlink():
                    continue
                marker = self.folder / (
                    ".sugarcubes-"
                    + hashlib.sha256(candidate.name.casefold().encode()).hexdigest()
                    + ".reserve"
                )
                try:
                    os.link(token_file, marker)
                except FileExistsError:
                    continue
                self._marker = marker
                if candidate.exists() or candidate.is_symlink():
                    self.release()
                    continue
                return
            raise ValueError("Could not reserve a side-by-side model destination.")
        finally:
            token_file.unlink(missing_ok=True)

    def publish(self, temporary: Path) -> str:
        """Reveal complete bytes with an exclusive hard link that cannot replace another writer."""
        self.validate()
        if not self._owns_marker():
            raise ValueError(
                "The model destination reservation changed before publication."
            )
        try:
            os.link(temporary, self._target)
        except FileExistsError as error:
            raise ValueError(
                "A model with this filename already exists; the installed file was preserved."
            ) from error
        self._committed = True
        return str(self._target.relative_to(self._root))

    def validate(self) -> None:
        """Reject linked directory escapes before reserving, writing, reusing or publishing."""
        if not self._target.resolve().is_relative_to(self._root):
            raise ValueError("The model download target escaped its approved root.")
        parent = self._target.parent
        while parent != self._root:
            if parent.is_symlink() or parent.is_junction():
                raise ValueError(
                    "The model download target contains a linked directory."
                )
            parent = parent.parent

    def release(self) -> None:
        """Remove only a lease whose bounded marker still belongs to this transaction."""
        if self._owns_marker() and self._marker is not None:
            self._marker.unlink(missing_ok=True)
        self._marker = None

    def _owns_marker(self) -> bool:
        """Require exact token bytes rather than trusting a path another writer can replace."""
        if self._marker is None or self._marker.is_symlink():
            return False
        try:
            with self._marker.open("rb") as stream:
                return stream.read(len(self._token) + 1) == self._token
        except FileNotFoundError:
            return False
