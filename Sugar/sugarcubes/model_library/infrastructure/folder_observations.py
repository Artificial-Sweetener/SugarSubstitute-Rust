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
"""Detect stable model-folder changes without reading model contents."""

from __future__ import annotations

from collections.abc import Callable
import logging
import os
from pathlib import Path
from time import monotonic

from ..application.ports import ModelRoots
from ..domain.models import ModelEvidence

_LOGGER = logging.getLogger(__name__)
DirectorySignature = dict[str, tuple[tuple[str, int], ...]]


class ModelFolderObservations:
    """Own cheap directory checks, occasional known-file stats, and copy debounce."""

    def __init__(
        self,
        roots: ModelRoots,
        clock: Callable[[], float] = monotonic,
        safety_interval: float = 180,
    ) -> None:
        """Use the approved registry and a monotonic clock for bounded idle work."""
        self._roots = roots
        self._clock = clock
        self._safety_interval = safety_interval
        self._directories: DirectorySignature | None = None
        self._known: tuple[ModelEvidence, ...] = ()
        self._pending: tuple[ModelEvidence, ...] | None = None
        self._pending_kinds: tuple[str, ...] = ()
        self._last_safety = 0.0

    def establish(self, evidence: tuple[ModelEvidence, ...]) -> None:
        """Adopt the already committed initial catalog without another full scan."""
        self._known = evidence
        self._directories = self._directory_signature()
        self._last_safety = self._clock()

    def poll(self) -> tuple[str, ...]:
        """Return dirty kinds only after two matching scans on separate polling turns."""
        current = self._directory_signature()
        previous = self._directories
        if previous is None:
            raise RuntimeError("Model folder observations need an initial catalog.")
        dirty = {
            kind
            for kind in current.keys() | previous.keys()
            if current.get(kind) != previous.get(kind)
        }
        dirty.update(self._pending_kinds)
        now = self._clock()
        if now - self._last_safety >= self._safety_interval:
            dirty.update(self._changed_known_files())
            self._last_safety = now
        if not dirty:
            return ()
        kinds = tuple(sorted(dirty))
        observed = tuple(
            sorted(self._roots.list_files(kinds), key=lambda item: item.location_id)
        )
        if observed != self._pending or kinds != self._pending_kinds:
            self._pending, self._pending_kinds = observed, kinds
            return ()
        self._known = (
            tuple(item for item in self._known if item.kind not in dirty) + observed
        )
        self._directories = current
        self._pending, self._pending_kinds = None, ()
        return kinds

    def retry(self, kinds: tuple[str, ...]) -> None:
        """Re-observe a failed refresh rather than accepting an unpublished generation."""
        self._pending_kinds = tuple(sorted(set(self._pending_kinds) | set(kinds)))
        self._pending = None

    def retry_all(self) -> None:
        """Recover a failed cold scan even when the index has no known file evidence yet."""
        self.retry(tuple(self._roots.configured_folders()))

    def _changed_known_files(self) -> set[str]:
        """Catch in-place writes whose parent directory timestamp stays unchanged."""
        changed: set[str] = set()
        for entry in self._known:
            path = self._roots.resolve_file(entry.kind, entry.value)
            try:
                stat = Path(path).stat() if path is not None else None
            except OSError:
                stat = None
            if stat is None or (stat.st_size, stat.st_mtime_ns) != (
                entry.size_bytes,
                entry.modified_ns,
            ):
                changed.add(entry.kind)
        return changed

    def _directory_signature(self) -> DirectorySignature:
        """Ignore library internals and linked subtrees during cheap directory traversal."""
        result: DirectorySignature = {}
        for kind, roots in self._roots.configured_folders().items():
            directories: dict[str, int] = {}
            for root in roots:
                if not Path(root).is_dir():
                    continue
                for directory, children, _ in os.walk(
                    root, followlinks=False, onerror=_walk_error
                ):
                    children[:] = [
                        child
                        for child in children
                        if child not in (".git", ".sugarcubes")
                        and not Path(directory, child).is_symlink()
                        and not Path(directory, child).is_junction()
                    ]
                    try:
                        directories[directory] = Path(directory).stat().st_mtime_ns
                    except OSError:
                        _LOGGER.warning(
                            "Model directory changed during observation",
                            extra={"kind": kind},
                            exc_info=True,
                        )
            result[kind] = tuple(sorted(directories.items()))
        return result


def _walk_error(error: OSError) -> None:
    """Keep permission and disappearance failures observable without reading weights."""
    _LOGGER.warning(
        "Model folder observation could not read a directory",
        exc_info=(type(error), error, error.__traceback__),
    )
