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
"""Confirm original encoded bytes through stable regular handles and approved output namespaces."""

from collections.abc import Mapping
import hashlib
import logging
import os
from pathlib import Path
import stat

from ..application.artifact_service import ArtifactReadBudget
from ..domain.artifact import (
    ArtifactReceipt,
    ArtifactUnavailable,
    ExpectedArtifact,
    same_artifact,
)
from ..domain.transfer import OutputFileReference
from .artifact_budget import ArtifactBudget
from .artifact_metadata import image_metadata
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX, ManagedArtifactRegistry
from .settings_paths import resolve_output_root
from .contained_files import (
    approved_directory,
    contained_file,
    open_contained,
    scan_directory,
)

_LOGGER = logging.getLogger(__name__)
_SUPPORTED = {".png", ".apng", ".jpg", ".jpeg", ".webp", ".gif", ".mp4", ".webm"}


class HostArtifactFiles:
    """Keep root authorization, exact file reads and bounded search independent of session membership."""

    def __init__(
        self, registry: ManagedArtifactRegistry, roots: Mapping[str, Path]
    ) -> None:
        """Bind only host-provided roots and the existing opaque managed artifact authority."""
        self._registry = registry
        self._roots = {kind: resolve_output_root(root) for kind, root in roots.items()}

    def receipt(
        self, file: OutputFileReference, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Inspect one qualified original; convert expected file failures into per-item outcomes."""
        if not isinstance(budget, ArtifactBudget):
            raise TypeError("Artifact files require a charged filesystem budget.")
        try:
            root, path = self._location(file)
            return self._read(root, path, file, budget)
        except FileNotFoundError as error:
            raise ArtifactUnavailable(
                "missing", "The recorded output file is missing."
            ) from error
        except PermissionError as error:
            raise ArtifactUnavailable(
                "inaccessible", "The recorded output file cannot be read."
            ) from error
        except ArtifactUnavailable:
            raise
        except ValueError as error:
            raise ArtifactUnavailable(
                "invalid_reference", "The output file is outside its approved location."
            ) from error
        except OSError as error:
            raise ArtifactUnavailable(
                "inaccessible", "The recorded output file cannot be inspected."
            ) from error

    def _location(self, file: OutputFileReference) -> tuple[Path, Path]:
        """Delegate opaque roots to the registry; ordinary descriptors never select a root."""
        if file.subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
            identity = file.subfolder[len(MANAGED_SUBFOLDER_PREFIX) :]
            path = self._registry.resolve(identity, file.filename, file.type)
            if path is None:
                raise ArtifactUnavailable(
                    "missing", "The managed output file is unavailable."
                )
            # Registry containment has already qualified its approved external ancestry.
            return path.parent, path
        root = self._roots.get(file.type)
        if root is None or not root.is_absolute():
            raise ArtifactUnavailable(
                "invalid_reference", "The output namespace has no approved root."
            )
        return root, root / file.subfolder / file.filename

    @staticmethod
    def _read(
        root: Path, path: Path, file: OutputFileReference, budget: ArtifactBudget
    ) -> ArtifactReceipt:
        """Hash bounded chunks and inspect headers using the same pinned, unchanged original handle."""
        if path.suffix.casefold() not in _SUPPORTED:
            raise ArtifactUnavailable(
                "unsupported", "This output media format cannot be restored."
            )
        with open_contained(root, path) as stream:
            before = os.fstat(stream.fileno())
            if before.st_size < 1 or before.st_size > 8589934592:
                raise ArtifactUnavailable(
                    "budget", "The output file exceeds the supported size limit."
                )
            digest = hashlib.sha256()
            total = 0
            tail = b""
            prefix = b""
            while True:
                budget.check()
                chunk = stream.read(32768)
                if not chunk:
                    break
                if len(prefix) < 12:
                    prefix += chunk[: 12 - len(prefix)]
                digest.update(chunk)
                total += len(chunk)
                tail = (tail + chunk)[-12:]
                if total > before.st_size:
                    raise ArtifactUnavailable(
                        "changed", "The output file changed while being read."
                    )
            probe = image_metadata(stream, budget, path.suffix.casefold())
            complete = (
                (probe.mime == "image/png" and tail.endswith(b"IEND\xaeB`\x82"))
                or (probe.mime == "image/jpeg" and tail.endswith(b"\xff\xd9"))
                or (
                    probe.mime == "image/webp"
                    and prefix[:4] == b"RIFF"
                    and len(prefix) == 12
                    and int.from_bytes(prefix[4:8], "little") + 8 == total
                )
            )
            complete = (
                complete
                or (probe.mime == "image/gif" and tail.endswith(b";"))
                or probe.media == "video"
            )
            after = os.fstat(stream.fileno())
            if not complete:
                raise ArtifactUnavailable(
                    "unsupported", "The output image is incomplete."
                )
            if total != before.st_size or (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            ) != (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            ):
                raise ArtifactUnavailable(
                    "changed", "The output file changed while being inspected."
                )
            budget.check()
            return ArtifactReceipt(
                file,
                digest.hexdigest(),
                total,
                probe.width,
                probe.height,
                probe.mime,
                probe.embedded,
                media=probe.media,
                embedded_prompt_id=probe.prompt_id,
                animated=probe.animated,
            )

    def rediscover(
        self, expected: ExpectedArtifact, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Search an ordinary descriptor's current root without following links or guessing provenance."""
        if not isinstance(budget, ArtifactBudget):
            raise TypeError("Artifact files require a charged filesystem budget.")
        file = expected.receipt.file
        if file.subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
            raise ArtifactUnavailable("missing", "The managed output file is missing.")
        root = self._roots.get(file.type)
        if root is None or not root.is_absolute():
            raise ArtifactUnavailable(
                "invalid_reference", "The output namespace has no approved root."
            )
        try:
            # Check root ancestry without inventing a candidate path or following a linked root.
            approved_directory(root)
            return self._search(root, expected, budget)
        except FileNotFoundError as error:
            raise ArtifactUnavailable(
                "missing", "The approved output folder is missing."
            ) from error
        except ValueError as error:
            if isinstance(error, ArtifactUnavailable):
                raise
            raise ArtifactUnavailable(
                "invalid_reference", "The output folder contains a linked location."
            ) from error
        except OSError as error:
            raise ArtifactUnavailable(
                "inaccessible", "The output folder cannot be inspected."
            ) from error

    def _search(
        self, root: Path, expected: ExpectedArtifact, budget: ArtifactBudget
    ) -> ArtifactReceipt:
        """Enumerate within one finite operation and compare bytes rather than names or timestamps."""
        directories = [root]
        found: ArtifactReceipt | None = None
        while directories:
            budget.check()
            directory = directories.pop()
            with scan_directory(directory) as entries:
                for entry in entries:
                    budget.candidate()
                    info = entry.stat(follow_symlinks=False)
                    if (
                        stat.S_ISLNK(info.st_mode)
                        or getattr(info, "st_file_attributes", 0) & 0x400
                    ):
                        continue
                    path = directory / entry.name
                    if stat.S_ISDIR(info.st_mode):
                        directories.append(path)
                        continue
                    if (
                        not stat.S_ISREG(info.st_mode)
                        or info.st_size != expected.receipt.bytes
                        or path.suffix.casefold() not in _SUPPORTED
                    ):
                        continue
                    contained_file(root, path)
                    file = OutputFileReference(
                        path.name,
                        (
                            ""
                            if path.parent == root
                            else path.parent.relative_to(root).as_posix()
                        ),
                        expected.receipt.file.type,
                    )
                    try:
                        actual = self.receipt(file, budget)
                    except ArtifactUnavailable as error:
                        if error.code in {"budget", "cancelled", "invalid_reference"}:
                            raise
                        _LOGGER.debug(
                            "Output discovery candidate is unavailable.",
                            extra={"outcome": error.code},
                        )
                        continue
                    if (
                        expected.prompt_id is not None
                        and actual.embedded_prompt_id is not None
                        and actual.embedded_prompt_id != expected.prompt_id
                    ):
                        continue
                    if same_artifact(expected.receipt, actual) and (
                        found is None
                        or (file.subfolder, file.filename)
                        < (found.file.subfolder, found.file.filename)
                    ):
                        found = actual
        if found is None:
            raise ArtifactUnavailable(
                "missing", "The exact recorded output could not be found."
            )
        return found
