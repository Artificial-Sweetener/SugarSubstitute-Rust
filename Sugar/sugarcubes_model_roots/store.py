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
"""Persist the Comfy installation's authoritative model-root selection."""

from __future__ import annotations

import json
import os
import stat
from enum import Enum
from pathlib import Path
from uuid import uuid4

_CONFIG_DIRECTORY = ".substitute"
_CONFIG_FILE = "model_root.json"
_SCHEMA_VERSION = 1
_PRESERVED_RECORD_MAX_BYTES = 256 * 1024


class ModelRootSelectionSource(str, Enum):
    """Identify the authoritative record considered by a preservation read."""

    CURRENT = "current"
    LEGACY = "legacy"


class ModelRootSelectionFailure(str, Enum):
    """Classify preservation refusal without disclosing configuration bytes."""

    UNSUPPORTED_SCHEMA = "unsupported_schema"
    CORRUPT = "corrupt"
    CONFLICT = "conflict"
    UNREADABLE = "unreadable"


class ModelRootSelectionError(ValueError):
    """Retain a failed record's identity and chain its original decoding or IO cause."""

    def __init__(
        self,
        reason: ModelRootSelectionFailure,
        source: ModelRootSelectionSource,
        path: Path,
    ) -> None:
        """Expose typed refusal metadata without putting user paths or bytes in the message."""
        super().__init__(
            f"Preserved {source.value} model-root selection is {reason.value}."
        )
        self.reason = reason
        self.source = source
        self.path = path


class _RecordError(ValueError):
    """Share codec rejection classification with existing configuration consumers."""

    def __init__(self, reason: ModelRootSelectionFailure, message: str) -> None:
        """Preserve established configuration diagnostics and attach a machine reason."""
        super().__init__(message)
        self.reason = reason


class _RootConflict(ValueError):
    """Distinguish an existing nondirectory from malformed configured path text."""


class ModelRootStore:
    """Read and atomically replace model-root configuration for one Comfy root."""

    def __init__(self, comfy_root: Path) -> None:
        """Initialize persistence beneath the supplied Comfy installation."""

        self._comfy_root = comfy_root.resolve()
        self._config_path = self._comfy_root / _CONFIG_DIRECTORY / _CONFIG_FILE
        self._legacy_path = (
            self._comfy_root / _CONFIG_DIRECTORY / "managed_model_root.json"
        )

    @property
    def config_path(self) -> Path:
        """Return the canonical persisted configuration path."""

        return self._config_path

    @property
    def default_root(self) -> Path:
        """Return the owning installation's normalized default model directory."""
        return (self._comfy_root / "models").resolve()

    def load(self) -> Path | None:
        """Return the configured custom root, or ``None`` for Comfy's default."""

        if not self._config_path.exists():
            return None
        return self._read_record(self._config_path, ModelRootSelectionSource.CURRENT)

    def read_preserved_selection(self) -> Path | None:
        """Read current or legacy intent without writing, creating, renaming or migrating.

        Current presence is authoritative. A valid current record returns immediately,
        without reading even corrupt or conflicting legacy state; an invalid current
        record never falls back. Only when current is absent is legacy considered.
        ``None`` means both records are absent, so the caller may use ``default_root``.
        Path validation stays with ``resolve_custom_root``. This read grants no native
        file custody; the caller retains ownership and preservation of original bytes.
        Each selected record is limited to 256 KiB of encoded UTF-8 JSON before decode,
        including when the file grows after inspection. This accommodates a maximum
        Windows path even when JSON escapes every UTF-16 code unit.

        Raises:
            ModelRootSelectionError: A selected record has an unsupported schema,
                corrupt contents, unreadable storage or a filesystem conflict. The
                original cause is chained and original files remain untouched.
        """
        current = self._read_preserved_record(
            self._config_path, ModelRootSelectionSource.CURRENT
        )
        if current is not None:
            return current
        return self._read_preserved_record(
            self._legacy_path, ModelRootSelectionSource.LEGACY
        )

    def _read_preserved_record(
        self, path: Path, source: ModelRootSelectionSource
    ) -> Path | None:
        """Separate genuine absence from selected-record failures before decoding."""
        try:
            if not self._record_present(path):
                return None
            if not stat.S_ISREG(path.stat().st_mode):
                raise _RootConflict("Model-root configuration must be a regular file.")
            with path.open("rb") as record:
                contents = record.read(_PRESERVED_RECORD_MAX_BYTES + 1)
            if len(contents) > _PRESERVED_RECORD_MAX_BYTES:
                raise _RecordError(
                    ModelRootSelectionFailure.CORRUPT,
                    "Preserved model-root configuration exceeds 256 KiB.",
                )
            return self._decode_record(contents.decode("utf-8"), source)
        except _RecordError as error:
            raise ModelRootSelectionError(error.reason, source, path) from error
        except RecursionError as error:
            raise ModelRootSelectionError(
                ModelRootSelectionFailure.CORRUPT, source, path
            ) from error
        except (_RootConflict, NotADirectoryError, RuntimeError) as error:
            raise ModelRootSelectionError(
                ModelRootSelectionFailure.CONFLICT, source, path
            ) from error
        except ValueError as error:
            raise ModelRootSelectionError(
                ModelRootSelectionFailure.CORRUPT, source, path
            ) from error
        except OSError as error:
            raise ModelRootSelectionError(
                ModelRootSelectionFailure.UNREADABLE, source, path
            ) from error

    def _record_present(self, path: Path) -> bool:
        """Distinguish absent records from colliding namespaces and broken selected links."""
        try:
            root = self._comfy_root.stat()
        except FileNotFoundError:
            return False
        if not stat.S_ISDIR(root.st_mode):
            raise _RootConflict("The Comfy configuration root must be a directory.")
        try:
            path.parent.lstat()
        except FileNotFoundError:
            return False
        if not stat.S_ISDIR(path.parent.stat().st_mode):
            raise _RootConflict(
                "The model-root configuration namespace must be a directory."
            )
        try:
            path.lstat()
        except FileNotFoundError:
            return False
        return True

    def _read_record(self, path: Path, source: ModelRootSelectionSource) -> Path:
        """Retain established loading behavior while sharing the authoritative codec."""
        return self._decode_record(path.read_text(encoding="utf-8"), source)

    def _decode_record(self, contents: str, source: ModelRootSelectionSource) -> Path:
        """Decode both historical spellings through the one model-root path owner."""
        legacy = source is ModelRootSelectionSource.LEGACY
        schema_key, root_key = (
            ("schema_version", "model_root")
            if legacy
            else ("schemaVersion", "modelRoot")
        )
        unsupported = (
            "Unsupported legacy model-root configuration."
            if legacy
            else "Unsupported model-root configuration schema."
        )
        payload: object = json.loads(contents)
        if not isinstance(payload, dict):
            raise _RecordError(ModelRootSelectionFailure.CORRUPT, unsupported)
        if payload.get(schema_key) != _SCHEMA_VERSION:
            raise _RecordError(
                ModelRootSelectionFailure.UNSUPPORTED_SCHEMA, unsupported
            )
        value: object = payload.get(root_key)
        if not isinstance(value, str) or not value.strip():
            raise _RecordError(
                ModelRootSelectionFailure.CORRUPT,
                (
                    "Legacy model-root configuration is invalid."
                    if legacy
                    else "Model-root configuration does not contain a path."
                ),
            )
        return self.resolve_custom_root(value)

    def save(self, model_root: Path | None) -> Path | None:
        """Persist a custom root, or remove configuration for Comfy's default."""

        if self._config_path.exists():
            self.load()
        if model_root is None:
            self._remove_config()
            return None
        resolved_root = self.resolve_custom_root(str(model_root))
        resolved_root.mkdir(parents=True, exist_ok=True)
        if not resolved_root.is_dir():
            raise ValueError("Model root must be a directory.")
        self._config_path.parent.mkdir(parents=True, exist_ok=True)
        temporary_path = self._config_path.with_name(
            f".{self._config_path.name}.{uuid4().hex}.tmp"
        )
        try:
            temporary_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": _SCHEMA_VERSION,
                        "modelRoot": str(resolved_root),
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            os.replace(temporary_path, self._config_path)
        finally:
            if temporary_path.exists():
                temporary_path.unlink()
        return resolved_root

    @staticmethod
    def resolve_custom_root(value: str) -> Path:
        """Validate and normalize one host-side custom model root."""

        expanded = Path(os.path.expandvars(value)).expanduser()
        if not expanded.is_absolute():
            raise ValueError("Model root must be an absolute path.")
        resolved = expanded.resolve()
        if resolved.exists() and not resolved.is_dir():
            raise _RootConflict("Model root must be a directory.")
        return resolved

    def migrate_legacy(self) -> None:
        """Adopt historical desktop state while retaining recoverable original bytes."""
        legacy = self._legacy_path
        if not legacy.exists():
            return
        if not self._config_path.exists():
            self.save(self._read_record(legacy, ModelRootSelectionSource.LEGACY))
        self.load()
        backup = legacy.with_name(f"{legacy.name}.migrated-{uuid4().hex}")
        os.replace(legacy, backup)

    def _remove_config(self) -> None:
        """Remove the custom selection while preserving unrelated host state."""

        if self._config_path.exists():
            self._config_path.unlink()
        directory = self._config_path.parent
        if directory.exists() and not any(directory.iterdir()):
            directory.rmdir()


__all__ = [
    "ModelRootStore",
    "ModelRootSelectionError",
    "ModelRootSelectionFailure",
    "ModelRootSelectionSource",
]
