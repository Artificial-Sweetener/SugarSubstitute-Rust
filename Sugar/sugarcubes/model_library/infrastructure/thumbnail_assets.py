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
"""Publish prepared content-addressed images beneath the model library root."""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Callable, Sequence
import hashlib
import os
import stat
from pathlib import Path
import tempfile

from .metadata_codec import normalized_hash
from .thumbnail_sources import SqliteThumbnailSources
from ..domain.models import ThumbnailReference
from ..domain.thumbnail_sources import ThumbnailOwnership, ThumbnailSourceSummary


@dataclass(frozen=True, slots=True)
class PreparedAsset:
    """Carry approved prepared-image metadata captured by its authoritative asset owner."""

    asset_id: str
    path: Path
    content_type: str
    size_bytes: int
    modified_ns: int


class ModelThumbnailAssets:
    """Own atomic publication and safe opaque-ID lookup of thumbnail bytes."""

    def __init__(self, asset_root: Path) -> None:
        """Prepare one namespace owned exclusively by the model library."""
        self._root = asset_root.resolve()
        self._root.mkdir(parents=True, exist_ok=True)
        self._sources = SqliteThumbnailSources(
            self._root / "sources.sqlite3", self._size, self._remove
        )

    def prepare_source(
        self,
        origin: str,
        source_id: str,
        generate: Callable[[], tuple[ThumbnailReference, ...]],
    ) -> tuple[ThumbnailReference, ...]:
        """Publish an explicitly owned image source without racing cache collection."""
        return self._sources.prepare(origin, source_id, generate)

    def source_summary(self, origin: str) -> ThumbnailSourceSummary:
        """Read source usage through its authoritative ownership repository."""
        return self._sources.summary(origin)

    def retain_sources(self, ownership: Sequence[ThumbnailOwnership]) -> None:
        """Protect persisted image choices before provider cache assets can be collected."""
        self._sources.retain(ownership)

    def clear_sources(self, origin: str) -> None:
        """Release one origin while preserving variants shared with other owners."""
        self._sources.clear(origin)

    def _size(self, identity: str) -> int | None:
        """Read only an approved prepared file's stat evidence."""
        asset = self.resolve(identity)
        return asset.size_bytes if asset is not None else None

    def _remove(self, identity: str) -> None:
        """Collect an opaque prepared identity without following namespace escapes."""
        path = self._root / f"{normalized_hash(identity)}.webp"
        if not path.resolve().is_relative_to(self._root):
            raise ValueError("Prepared thumbnail escapes its asset namespace.")
        path.unlink(missing_ok=True)

    def publish(self, content: bytes) -> str:
        """Publish a prepared WebP once and make partial writes invisible to readers."""
        if len(content) < 12 or content[:4] != b"RIFF" or content[8:12] != b"WEBP":
            raise ValueError("Prepared thumbnail assets must be WebP images.")
        asset_id = hashlib.sha256(content).hexdigest()
        destination = self._root / f"{asset_id}.webp"
        if destination.exists():
            return asset_id
        descriptor, name = tempfile.mkstemp(prefix=".pending-", dir=self._root)
        temporary = Path(name)
        try:
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(content)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, destination)
        finally:
            temporary.unlink(missing_ok=True)
        return asset_id

    def resolve(self, asset_id: str) -> PreparedAsset | None:
        """Resolve an approved regular file with one authoritative size and timestamp snapshot."""
        identity = normalized_hash(asset_id)
        path = self._root / f"{identity}.webp"
        if not path.resolve().is_relative_to(self._root):
            return None
        try:
            evidence = path.stat()
        except FileNotFoundError:
            return None
        if not stat.S_ISREG(evidence.st_mode):
            return None
        return PreparedAsset(
            identity, path, "image/webp", evidence.st_size, evidence.st_mtime_ns
        )
