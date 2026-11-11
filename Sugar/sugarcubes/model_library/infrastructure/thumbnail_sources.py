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
"""Serialize prepared-source ownership and cache collection across library instances."""

from __future__ import annotations

from collections.abc import Callable, Iterator, Sequence
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import sqlite3

from ..domain.models import ThumbnailReference
from ..domain.thumbnail_sources import (
    ThumbnailOwnership,
    ThumbnailSourceSummary,
    thumbnail_source_origin,
)
from .metadata_codec import normalized_hash


class SqliteThumbnailSources:
    """Own source-to-variant relations while delegating physical image access."""

    def __init__(
        self,
        path: Path,
        size: Callable[[str], int | None],
        remove: Callable[[str], None],
    ) -> None:
        """Bind one prepared-asset namespace and preserve unsupported ownership schemas."""
        self._path = path
        self._size = size
        self._remove = remove
        with self._connect() as connection:
            version = connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, 1):
                raise ValueError(
                    "Unsupported thumbnail source schema; original data was preserved."
                )
            connection.executescript("""
                CREATE TABLE IF NOT EXISTS thumbnail_sources (
                    origin TEXT NOT NULL, source_id TEXT NOT NULL,
                    PRIMARY KEY(origin,source_id)
                );
                CREATE TABLE IF NOT EXISTS thumbnail_source_variants (
                    origin TEXT NOT NULL, source_id TEXT NOT NULL, ordinal INTEGER NOT NULL,
                    asset_id TEXT NOT NULL, role TEXT NOT NULL, width INTEGER NOT NULL, height INTEGER NOT NULL,
                    PRIMARY KEY(origin,source_id,ordinal),
                    FOREIGN KEY(origin,source_id) REFERENCES thumbnail_sources(origin,source_id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS thumbnail_source_asset ON thumbnail_source_variants(asset_id);
                PRAGMA user_version=1;
            """)

    def prepare(
        self,
        origin: str,
        source_id: str,
        generate: Callable[[], tuple[ThumbnailReference, ...]],
    ) -> tuple[ThumbnailReference, ...]:
        """Hold publication and ownership in one write lease so collection cannot interleave."""
        selected_origin = thumbnail_source_origin(origin)
        identity = normalized_hash(source_id)
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            stored = self._references(connection, selected_origin, identity)
            if stored and all(self._size(item.asset_id) is not None for item in stored):
                return stored
            references = generate()
            if not references:
                raise ValueError("Prepared thumbnail source must contain variants.")
            for item in references:
                _validate_reference(item)
                if (
                    item.content_type != "image/webp"
                    or self._size(item.asset_id) is None
                ):
                    raise ValueError(
                        "Prepared thumbnail source references unavailable or invalid assets."
                    )
            _replace_source(connection, selected_origin, identity, references)
            return references

    def retain(self, ownership: Sequence[ThumbnailOwnership]) -> None:
        """Backfill known persisted references without decoding files or duplicating source usage."""
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            retained: dict[str, set[str]] = {}
            for origin, asset_id in connection.execute(
                "SELECT DISTINCT origin,asset_id FROM thumbnail_source_variants"
            ):
                selected_origin = thumbnail_source_origin(origin)
                retained.setdefault(selected_origin, set()).add(_identity(asset_id))
            for source in ownership:
                selected_origin = thumbnail_source_origin(source.origin)
                for item in source.thumbnails:
                    _validate_reference(item)
                identities = {
                    normalized_hash(item.asset_id) for item in source.thumbnails
                }
                known = retained.setdefault(selected_origin, set())
                if identities.issubset(known):
                    continue
                signature = json.dumps(
                    [
                        (item.asset_id, item.role, item.width, item.height)
                        for item in source.thumbnails
                    ],
                    separators=(",", ":"),
                )
                source_id = hashlib.sha256(
                    b"retained-thumbnail-references-v1\0" + signature.encode("utf-8")
                ).hexdigest()
                _replace_source(
                    connection, selected_origin, source_id, source.thumbnails
                )
                known.update(identities)

    def summary(self, origin: str) -> ThumbnailSourceSummary:
        """Count actual physical bytes once, excluding references whose files disappeared."""
        selected_origin = thumbnail_source_origin(origin)
        with self._connect() as connection:
            rows = tuple(
                connection.execute(
                    "SELECT source_id,asset_id FROM thumbnail_source_variants WHERE origin=?",
                    (selected_origin,),
                )
            )
        sources: set[str] = set()
        sizes: dict[str, int] = {}
        for source_id, asset_id in rows:
            source = _identity(source_id)
            identity = _identity(asset_id)
            size = self._size(identity)
            if size is not None:
                sources.add(source)
                sizes[identity] = size
        return ThumbnailSourceSummary(len(sources), len(sizes), sum(sizes.values()))

    def clear(self, origin: str) -> None:
        """Collect only known variants with no remaining origin ownership."""
        selected_origin = thumbnail_source_origin(origin)
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            candidates = {
                _identity(row[0])
                for row in connection.execute(
                    "SELECT DISTINCT asset_id FROM thumbnail_source_variants WHERE origin=?",
                    (selected_origin,),
                )
            }
            connection.execute(
                "DELETE FROM thumbnail_sources WHERE origin=?", (selected_origin,)
            )
            retained = {
                _identity(row[0])
                for row in connection.execute(
                    "SELECT DISTINCT asset_id FROM thumbnail_source_variants"
                )
            }
            for identity in sorted(candidates - retained):
                self._remove(identity)

    def _references(
        self, connection: sqlite3.Connection, origin: str, source_id: str
    ) -> tuple[ThumbnailReference, ...]:
        """Validate stored variant geometry before returning it to the image pipeline."""
        references: list[ThumbnailReference] = []
        for asset_id, role, width, height in connection.execute(
            "SELECT asset_id,role,width,height FROM thumbnail_source_variants WHERE origin=? AND source_id=? ORDER BY ordinal",
            (origin, source_id),
        ):
            if (
                not isinstance(role, str)
                or role not in ("standard", "banner")
                or not isinstance(width, int)
                or width <= 0
                or not isinstance(height, int)
                or height <= 0
            ):
                raise ValueError("Invalid stored thumbnail source variant.")
            references.append(
                ThumbnailReference(
                    _identity(asset_id), role, width, height, "image/webp"
                )
            )
        return tuple(references)

    @contextmanager
    def _connect(self) -> Iterator[sqlite3.Connection]:
        """Release bounded SQLite leases on success, decode failure or cache collection errors."""
        connection = sqlite3.connect(self._path, timeout=10)
        try:
            connection.execute("PRAGMA foreign_keys=ON")
            with connection:
                yield connection
        finally:
            connection.close()


def _identity(value: object) -> str:
    """Narrow persisted asset and source identities before filesystem callbacks."""
    if not isinstance(value, str):
        raise ValueError("Invalid stored thumbnail source identity.")
    return normalized_hash(value)


def _validate_reference(item: ThumbnailReference) -> None:
    """Reject invalid owned references before they can address physical assets."""
    normalized_hash(item.asset_id)
    if (
        item.role not in ("standard", "banner")
        or not isinstance(item.width, int)
        or isinstance(item.width, bool)
        or item.width <= 0
        or not isinstance(item.height, int)
        or isinstance(item.height, bool)
        or item.height <= 0
        or item.content_type not in ("image/webp", "image/png", "image/jpeg")
    ):
        raise ValueError("Invalid owned thumbnail reference.")


def _replace_source(
    connection: sqlite3.Connection,
    origin: str,
    source_id: str,
    references: tuple[ThumbnailReference, ...],
) -> None:
    """Commit one validated source relation inside its caller's publication lease."""
    connection.execute(
        "DELETE FROM thumbnail_sources WHERE origin=? AND source_id=?",
        (origin, source_id),
    )
    connection.execute(
        "INSERT INTO thumbnail_sources VALUES (?,?)", (origin, source_id)
    )
    connection.executemany(
        "INSERT INTO thumbnail_source_variants VALUES (?,?,?,?,?,?,?)",
        (
            (
                origin,
                source_id,
                ordinal,
                item.asset_id,
                item.role,
                item.width,
                item.height,
            )
            for ordinal, item in enumerate(references)
        ),
    )
