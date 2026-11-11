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
"""Publish downloaded artifacts atomically without replacing installed model files."""

from __future__ import annotations

import hashlib
import logging
import os
from pathlib import Path
import tempfile

from ..domain.artifact_safety import safe_artifact_name
from ..domain.provider_releases import ModelArtifactOffer
from .artifact_destinations import ModelArtifactDestination

_LOGGER = logging.getLogger(__name__)


class ModelArtifactFile:
    """Own the temporary file, streamed digest, and exclusive publication transaction."""

    def __init__(
        self,
        root: str,
        offer: ModelArtifactOffer,
        relative_path: str | None = None,
    ) -> None:
        """Validate the approved destination before creating temporary download storage."""
        if (
            not safe_artifact_name(offer.file_name)
            or not 0 < offer.size_bytes <= 128 * 1024**3
        ):
            raise ValueError("Invalid model artifact name or size.")
        self._destination = ModelArtifactDestination(root, offer, relative_path)
        self._offer = offer
        self._temporary: Path | None = None
        self._digest = hashlib.sha256()
        self._size = 0

    @property
    def committed_name(self) -> str | None:
        """Expose the publication commit point independently of async cancellation timing."""
        return self._destination.committed_name

    @property
    def bytes_written(self) -> int:
        """Expose streamed byte evidence to the single shared progress owner."""
        return self._size

    def reuse(self) -> str | None:
        """Return unchanged size-and-hash matching bytes in the reviewed destination folder."""
        self._destination.validate()
        if not self._destination.folder.is_dir():
            return None
        for candidate in self._destination.folder.iterdir():
            try:
                if (
                    not safe_artifact_name(candidate.name)
                    or candidate.suffix.casefold()
                    != Path(self._offer.file_name).suffix.casefold()
                    or candidate.is_symlink()
                    or not candidate.is_file()
                ):
                    continue
                before = candidate.stat()
                if before.st_size != self._offer.size_bytes:
                    continue
                with candidate.open("rb") as stream:
                    digest = hashlib.file_digest(stream, "sha256").hexdigest()
                after = candidate.stat()
                if (
                    digest == self._offer.sha256
                    and (before.st_ino, before.st_size, before.st_mtime_ns)
                    == (after.st_ino, after.st_size, after.st_mtime_ns)
                    and not candidate.is_symlink()
                ):
                    return self._destination.reuse(candidate)
            except OSError:
                _LOGGER.debug(
                    "Skipped an unreadable acquisition candidate", exc_info=True
                )
        return None

    def begin(self) -> None:
        """Create a unique temporary file only when its final filename is unoccupied."""
        self._destination.reserve()
        descriptor, name = tempfile.mkstemp(
            prefix=".sugarcubes-", suffix=".download", dir=self._destination.folder
        )
        os.close(descriptor)
        self._temporary = Path(name)

    def append(self, block: bytes) -> None:
        """Bound streamed bytes while maintaining a digest of the exact published contents."""
        if self._temporary is None:
            raise RuntimeError("Model artifact storage was not started.")
        self._destination.validate()
        if self._size + len(block) > self._offer.size_bytes:
            raise ValueError("The downloaded model exceeds its declared size.")
        with self._temporary.open("ab") as stream:
            stream.write(block)
        self._digest.update(block)
        self._size += len(block)

    def publish(self) -> str:
        """Verify SHA256 and atomically link a complete file into an unoccupied destination."""
        self._destination.validate()
        if self._temporary is not None and self._size != self._offer.size_bytes:
            raise ValueError("The downloaded model ended before its declared size.")
        if (
            self._temporary is None
            or not self._size
            or self._digest.hexdigest() != self._offer.sha256
        ):
            raise ValueError("The downloaded model failed SHA256 verification.")
        name = self._destination.publish(self._temporary)
        self.abort()
        return name

    def abort(self) -> None:
        """Remove only this transaction's temporary file after failure or cancellation."""
        try:
            if self._temporary is not None:
                self._temporary.unlink(missing_ok=True)
                self._temporary = None
        finally:
            self._destination.release()
