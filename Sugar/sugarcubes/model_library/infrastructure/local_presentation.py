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
"""Import bounded local model sidecars and previews without reading model weights."""

from __future__ import annotations

import hashlib
import json
import logging
from pathlib import Path
from collections.abc import Sequence
from PIL.Image import DecompressionBombError

from ..application.ports import ModelRoots
from .local_metadata_codec import parse_local_model_metadata
from .catalog_store import SqliteModelCatalogRepository
from .thumbnail_preparer import ModelThumbnailPreparer

_LOGGER = logging.getLogger(__name__)


class LocalModelPresentation:
    """Prepare reusable images only when adjacent source evidence changes."""

    def __init__(
        self,
        roots: ModelRoots,
        repository: SqliteModelCatalogRepository,
        images: ModelThumbnailPreparer,
    ) -> None:
        """Compose approved path resolution, presentation persistence, and image preparation."""
        self._roots = roots
        self._repository = repository
        self._images = images

    def refresh(self, kinds: Sequence[str] | None) -> int:
        """Refresh local presentation off the event loop with per-model fault isolation."""
        prepared = 0
        for entry in self._repository.list_entries(kinds):
            evidence = entry.evidence
            resolved = self._roots.resolve_file(evidence.kind, evidence.value)
            if resolved is None:
                continue
            model = Path(resolved)
            try:
                sidecar = next(
                    (
                        path
                        for suffix in (".json", ".civitai.info", ".info.json")
                        if (
                            path := _existing(
                                model.with_suffix(suffix), model.parent, 2 * 1024 * 1024
                            )
                        )
                        is not None
                    ),
                    None,
                )
                preview = next(
                    (
                        path
                        for extension in (
                            ".preview.png",
                            ".png",
                            ".jpg",
                            ".jpeg",
                            ".webp",
                        )
                        if (
                            path := _existing(
                                model.with_suffix(extension),
                                model.parent,
                                20 * 1024 * 1024,
                            )
                        )
                        is not None
                    ),
                    None,
                )
                signature = _signature(sidecar, preview)
                if self._repository.local_signature(evidence.location_id) == signature:
                    continue
                thumbnails = (
                    self._images.prepare(preview.read_bytes())
                    if preview is not None
                    else ()
                )
                payload: object = (
                    json.loads(sidecar.read_text(encoding="utf-8"))
                    if sidecar is not None
                    else {}
                )
                metadata = parse_local_model_metadata(payload, model.stem, thumbnails)
                prepared += self._repository.save_local_metadata(
                    evidence, signature, metadata
                )
            except (OSError, ValueError, DecompressionBombError):
                _LOGGER.exception(
                    "Local model presentation import failed",
                    extra={"model_id": evidence.location_id, "kind": evidence.kind},
                )
        return prepared


def _existing(candidate: Path, parent: Path, limit: int) -> Path | None:
    """Reject escaped symlinks and oversized adjacent metadata before reading bytes."""
    path = candidate.resolve()
    if path.parent != parent.resolve() or not path.is_file():
        return None
    if path.stat().st_size > limit:
        raise ValueError("Local model metadata exceeds the supported size limit.")
    return path


def _signature(sidecar: Path | None, preview: Path | None) -> str:
    """Hash small source observations without hashing or opening model contents."""
    observations: list[str] = ["local-presentation-v2"]
    for path in (sidecar, preview):
        if path is not None:
            stat = path.stat()
            observations.append(f"{path.name}:{stat.st_size}:{stat.st_mtime_ns}")
        else:
            observations.append("missing")
    return hashlib.sha256("\0".join(observations).encode("utf-8")).hexdigest()
