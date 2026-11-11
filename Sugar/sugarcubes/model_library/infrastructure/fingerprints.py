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
"""Hash explicit model requests with bounded memory and stale-file safeguards."""

from __future__ import annotations

import asyncio
import hashlib
from pathlib import Path
from threading import Event

from ..application.ports import ModelRoots
from ..domain.models import ModelEvidence


class ApprovedModelFingerprints:
    """Resolve every fingerprint through the host's approved model registry."""

    def __init__(self, roots: ModelRoots) -> None:
        """Bind path trust without caching stale physical filenames."""
        self._roots = roots

    async def fingerprint(self, evidence: ModelEvidence) -> str:
        """Stop background reads on cancellation and reject changed file evidence."""
        cancelled = Event()
        try:
            return await asyncio.to_thread(self._read, evidence, cancelled)
        except asyncio.CancelledError:
            cancelled.set()
            raise

    def _read(self, evidence: ModelEvidence, cancelled: Event) -> str:
        """Stream approved bytes without allocating model-sized buffers or loading weights."""
        resolved = self._roots.resolve_file(evidence.kind, evidence.value)
        if resolved is None:
            raise ValueError("The requested model is no longer available.")
        path = Path(resolved)
        before = path.stat()
        if (
            before.st_size != evidence.size_bytes
            or before.st_mtime_ns != evidence.modified_ns
        ):
            raise ValueError("The model changed before fingerprinting began.")
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            while content := stream.read(4 * 1024 * 1024):
                if cancelled.is_set():
                    raise ValueError("Model fingerprinting was cancelled.")
                digest.update(content)
        after = path.stat()
        if (after.st_size, after.st_mtime_ns, after.st_ino) != (
            before.st_size,
            before.st_mtime_ns,
            before.st_ino,
        ):
            raise ValueError("The model changed during fingerprinting.")
        return digest.hexdigest()
