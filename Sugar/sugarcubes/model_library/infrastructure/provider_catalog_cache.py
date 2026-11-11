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
"""Persist bounded provider catalog snapshots beneath the portable model library."""

from __future__ import annotations

from dataclasses import dataclass
import json
import logging
import math
from pathlib import Path
import uuid

_LOGGER = logging.getLogger(__name__)
_LIMIT = 12 * 1024 * 1024


@dataclass(frozen=True, slots=True)
class ProviderCatalogSnapshot:
    """Carry a validated cache envelope for provider-specific payload validation."""

    payload: object
    fetched_at: float
    etag: str | None
    last_modified: str | None = None


class ProviderCatalogCache:
    """Own atomic disposable remote snapshots without touching model or user metadata files."""

    def __init__(self, path: Path) -> None:
        """Bind a library-owned cache location supplied by runtime composition."""
        self._path = path

    def read(self) -> ProviderCatalogSnapshot | None:
        """Validate the bounded envelope and leave unsupported future schemas intact."""
        try:
            if self._path.stat().st_size > _LIMIT:
                raise ValueError("Provider cache exceeds its supported size limit.")
            value: object = json.loads(self._path.read_bytes())
            if not isinstance(value, dict):
                raise ValueError("Invalid provider catalog cache.")
            if value.get("schemaVersion") != 1:
                raise RuntimeError(
                    "Unsupported provider catalog cache schema; original data preserved."
                )
            timestamp = value.get("fetchedAt")
            etag = value.get("etag")
            last_modified = value.get("lastModified")
            if (
                not isinstance(timestamp, (int, float))
                or isinstance(timestamp, bool)
                or timestamp < 0
                or not math.isfinite(timestamp)
                or (etag is not None and not _valid_validator(etag))
                or (last_modified is not None and not _valid_validator(last_modified))
                or "payload" not in value
            ):
                raise ValueError("Invalid provider catalog cache fields.")
            return ProviderCatalogSnapshot(
                value["payload"], float(timestamp), etag, last_modified
            )
        except FileNotFoundError:
            return None
        except (OSError, ValueError) as error:
            _LOGGER.warning("Provider catalog cache is unreadable", exc_info=error)
            return None

    def save(self, snapshot: ProviderCatalogSnapshot) -> None:
        """Publish an atomic replacement after preserving the future-schema boundary."""
        self.read()
        if not math.isfinite(snapshot.fetched_at) or snapshot.fetched_at < 0:
            raise ValueError("Invalid provider catalog fetch timestamp.")
        if snapshot.etag is not None and not _valid_validator(snapshot.etag):
            raise ValueError("Invalid provider catalog ETag.")
        if snapshot.last_modified is not None and not _valid_validator(
            snapshot.last_modified
        ):
            raise ValueError("Invalid provider catalog Last-Modified validator.")
        content = json.dumps(
            {
                "schemaVersion": 1,
                "fetchedAt": snapshot.fetched_at,
                "etag": snapshot.etag,
                "lastModified": snapshot.last_modified,
                "payload": snapshot.payload,
            },
            ensure_ascii=True,
        ).encode("utf-8")
        if len(content) > _LIMIT:
            raise ValueError("Provider catalog cache exceeds its supported size limit.")
        self._path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self._path.with_name(
            self._path.name + "." + uuid.uuid4().hex + ".tmp"
        )
        try:
            temporary.write_bytes(content)
            temporary.replace(self._path)
        finally:
            temporary.unlink(missing_ok=True)


def _valid_validator(value: object) -> bool:
    """Reject header injection before cached validators return to HTTP adapters."""
    return (
        isinstance(value, str)
        and len(value) <= 1000
        and "\r" not in value
        and "\n" not in value
    )
