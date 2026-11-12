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
"""Read and write the small versioned JSON records the updater keeps on disk."""

from __future__ import annotations

import json
from collections.abc import Mapping
from pathlib import Path

from .errors import PackInstallError, PackInstallErrorCode
from .filesystem import durable_replace_text

RECORD_SCHEMA = 1
_MAX_RECORD_BYTES = 8 * 1024 * 1024


def read_record(path: Path) -> dict[str, object] | None:
    """Return one record's fields, `None` when absent, or raise when unusable."""

    try:
        with path.open("rb") as handle:
            content = handle.read(_MAX_RECORD_BYTES + 1)
    except FileNotFoundError:
        return None
    except OSError as exc:
        raise _corrupt(path) from exc
    if len(content) > _MAX_RECORD_BYTES:
        raise _corrupt(path)
    try:
        payload = json.loads(content.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise _corrupt(path) from exc
    if not isinstance(payload, dict) or payload.get("schema") != RECORD_SCHEMA:
        raise _corrupt(path)
    return payload


def write_record(path: Path, fields: Mapping[str, object]) -> None:
    """Durably replace one record, stamping the current schema."""

    durable_replace_text(
        path, json.dumps({"schema": RECORD_SCHEMA, **fields}, indent=2, sort_keys=True)
    )


def text_field(record: Mapping[str, object], key: str) -> str:
    """Return one required text field or fail as corrupt."""

    value = record.get(key)
    if not isinstance(value, str):
        raise PackInstallError(
            PackInstallErrorCode.STAGED_CORRUPT, f"Record field {key!r} is missing."
        )
    return value


def text_list_field(record: Mapping[str, object], key: str) -> tuple[str, ...]:
    """Return one required list of text or fail as corrupt."""

    value = record.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise PackInstallError(
            PackInstallErrorCode.STAGED_CORRUPT, f"Record field {key!r} is invalid."
        )
    return tuple(item for item in value if isinstance(item, str))


def text_map_field(record: Mapping[str, object], key: str) -> dict[str, str]:
    """Return one required text-to-text map or fail as corrupt."""

    value = record.get(key)
    if not isinstance(value, dict) or not all(
        isinstance(name, str) and isinstance(item, str) for name, item in value.items()
    ):
        raise PackInstallError(
            PackInstallErrorCode.STAGED_CORRUPT, f"Record field {key!r} is invalid."
        )
    return {str(name): str(item) for name, item in value.items()}


def _corrupt(path: Path) -> PackInstallError:
    """Build the failure for an unreadable record without exposing its path."""

    return PackInstallError(
        PackInstallErrorCode.STAGED_CORRUPT,
        f"The update record {path.name} is unreadable.",
    )


__all__ = [
    "RECORD_SCHEMA",
    "read_record",
    "text_field",
    "text_list_field",
    "text_map_field",
    "write_record",
]
