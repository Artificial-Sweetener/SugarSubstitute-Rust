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
"""Cache SugarCubes catalog snapshots behind explicit invalidation."""

from __future__ import annotations

import hashlib
import json
import logging
from dataclasses import dataclass
from typing import Any, Callable, Mapping, Sequence

_logger = logging.getLogger(__name__)

CatalogSummaryProvider = Callable[[bool], Sequence[Mapping[str, Any]]]
CatalogEntryBuilder = Callable[[Mapping[str, Any]], dict[str, Any]]
RevisionPackFactsProvider = Callable[[bool], Sequence[Mapping[str, Any]]]
PackCountsProvider = Callable[[], Mapping[str, int]]
TimestampProvider = Callable[[], str]


@dataclass(frozen=True)
class _CachedCatalogEntry:
    """Store one catalog entry with the cheap facts that validate reuse."""

    fingerprint: Mapping[str, Any]
    entry: Mapping[str, Any]


@dataclass(frozen=True)
class _CatalogSnapshot:
    """Represent one reusable catalog snapshot for a disabled-filter mode."""

    signature: str
    revision: str
    entries: tuple[Mapping[str, Any], ...]
    pack_counts: Mapping[str, int]


class CubeCatalogStateService:
    """Own cached catalog entries, revisions, and invalidation state.

    The cube library service remains the public facade. This owner centralizes
    the expensive catalog row construction so status, revision, and catalog
    calls reuse one snapshot while file stat facts prove source state unchanged.
    """

    def __init__(
        self,
        *,
        list_summaries: CatalogSummaryProvider,
        build_entry: CatalogEntryBuilder,
        revision_pack_facts: RevisionPackFactsProvider,
        pack_counts: PackCountsProvider,
        generated_at: TimestampProvider,
    ) -> None:
        """Initialize one process-local catalog state owner."""

        self._list_summaries = list_summaries
        self._build_entry = build_entry
        self._revision_pack_facts = revision_pack_facts
        self._pack_counts = pack_counts
        self._generated_at = generated_at
        self._snapshots: dict[bool, _CatalogSnapshot] = {}
        self._entry_cache: dict[str, _CachedCatalogEntry] = {}
        self._dirty_reason = "initial"

    def current_revision(self, *, include_disabled: bool = False) -> str:
        """Return the current deterministic catalog revision."""

        return self._snapshot(include_disabled=include_disabled).revision

    def current_catalog(self, *, include_disabled: bool = False) -> dict[str, Any]:
        """Return the current backend-facing catalog payload."""

        snapshot = self._snapshot(include_disabled=include_disabled)
        return {
            "schemaVersion": 1,
            "catalogRevision": snapshot.revision,
            "generatedAt": self._generated_at(),
            "cubes": [dict(entry) for entry in snapshot.entries],
            "packs": dict(snapshot.pack_counts),
        }

    def current_entries(
        self, *, include_disabled: bool = False
    ) -> tuple[Mapping[str, Any], ...]:
        """Return cached catalog entries for dependency consumers."""

        return self._snapshot(include_disabled=include_disabled).entries

    def invalidate(
        self,
        reason: str,
        *,
        affected_cube_ids: Sequence[str] = (),
    ) -> None:
        """Mark cached catalog state stale after a known mutation."""

        self._snapshots.clear()
        self._entry_cache.clear()
        self._dirty_reason = reason or "explicit"
        _logger.debug(
            "SugarCubes catalog state invalidated",
            extra={
                "reason": self._dirty_reason,
                "affected_cube_ids": list(affected_cube_ids),
            },
        )

    def _snapshot(self, *, include_disabled: bool) -> _CatalogSnapshot:
        """Return a cached snapshot or rebuild changed rows."""

        summaries = tuple(self._list_summaries(include_disabled))
        pack_facts = tuple(self._revision_pack_facts(include_disabled))
        signature = self._source_signature(
            include_disabled=include_disabled,
            summaries=summaries,
            pack_facts=pack_facts,
        )
        cached = self._snapshots.get(include_disabled)
        if cached is not None and cached.signature == signature:
            _logger.debug(
                "SugarCubes catalog state cache hit",
                extra={
                    "include_disabled": include_disabled,
                    "catalog_revision": cached.revision,
                },
            )
            return cached

        entries = tuple(
            sorted(
                (self._entry_for_summary(summary) for summary in summaries),
                key=_catalog_entry_sort_key,
            )
        )
        revision = self._catalog_revision(
            pack_facts=pack_facts,
            entries=entries,
        )
        snapshot = _CatalogSnapshot(
            signature=signature,
            revision=revision,
            entries=entries,
            pack_counts=dict(self._pack_counts()),
        )
        self._snapshots[include_disabled] = snapshot
        _logger.debug(
            "SugarCubes catalog state rebuilt",
            extra={
                "include_disabled": include_disabled,
                "catalog_revision": revision,
                "cube_count": len(entries),
                "reason": self._dirty_reason,
            },
        )
        self._dirty_reason = ""
        return snapshot

    def _entry_for_summary(self, summary: Mapping[str, Any]) -> Mapping[str, Any]:
        """Return a cached entry when one summary's stat facts are unchanged."""

        key = _summary_cache_key(summary)
        fingerprint = _summary_fingerprint(summary)
        cached = self._entry_cache.get(key)
        if cached is not None and cached.fingerprint == fingerprint:
            return cached.entry
        entry = self._build_entry(summary)
        self._entry_cache[key] = _CachedCatalogEntry(
            fingerprint=fingerprint,
            entry=dict(entry),
        )
        return entry

    def _source_signature(
        self,
        *,
        include_disabled: bool,
        summaries: Sequence[Mapping[str, Any]],
        pack_facts: Sequence[Mapping[str, Any]],
    ) -> str:
        """Return a cheap signature for the catalog-visible source state."""

        facts = {
            "include_disabled": include_disabled,
            "packs": list(pack_facts),
            "cubes": [_summary_fingerprint(summary) for summary in summaries],
        }
        return _stable_digest(facts)

    def _catalog_revision(
        self,
        *,
        pack_facts: Sequence[Mapping[str, Any]],
        entries: Sequence[Mapping[str, Any]],
    ) -> str:
        """Return the public revision from the same rows used for the catalog."""

        facts = {
            "packs": list(pack_facts),
            "cubes": [
                {
                    "cube_id": entry.get("cubeId"),
                    "version": entry.get("version"),
                    "content_hash": entry.get("contentHash"),
                    "source": entry.get("source"),
                }
                for entry in entries
            ],
        }
        return f"sha256:{_stable_digest(facts)}"


def _summary_cache_key(summary: Mapping[str, Any]) -> str:
    """Return a stable cache key for one summarized cube path."""

    path = str(summary.get("path") or "")
    cube_id = str(summary.get("cube_id") or "")
    return path or cube_id


def _summary_fingerprint(summary: Mapping[str, Any]) -> Mapping[str, Any]:
    """Return stat and source facts that validate a cached catalog row."""

    source = summary.get("source") if isinstance(summary.get("source"), Mapping) else {}
    return {
        "path": str(summary.get("path") or ""),
        "relative_path": str(summary.get("relative_path") or ""),
        "size_bytes": summary.get("size_bytes"),
        "mtime_ns": summary.get("mtime_ns"),
        "mtime": str(summary.get("mtime") or ""),
        "cube_id": str(summary.get("cube_id") or ""),
        "version": str(summary.get("version") or ""),
        "source": {
            "type": str(source.get("type") or summary.get("source_kind") or ""),
            "owner": str(source.get("owner") or summary.get("owner") or ""),
            "repo": str(source.get("repo") or summary.get("repo") or ""),
            "namespace": str(source.get("namespace") or summary.get("namespace") or ""),
            "repo_ref": str(source.get("repo_ref") or ""),
            "repo_relative_path": str(source.get("repo_relative_path") or ""),
        },
    }


def _catalog_entry_sort_key(entry: Mapping[str, Any]) -> tuple[str, str, str, str, str]:
    """Return the historical catalog sort key."""

    source = entry.get("source") if isinstance(entry.get("source"), Mapping) else {}
    return (
        str(source.get("kind", "")).casefold(),
        str(source.get("repoRef", "")).casefold(),
        str(entry.get("targetModel", "")).casefold(),
        str(entry.get("displayName", "")).casefold(),
        str(entry.get("cubeId", "")).casefold(),
    )


def _stable_digest(value: Mapping[str, Any]) -> str:
    """Return a stable SHA-256 digest for normalized catalog facts."""

    serialized = json.dumps(value, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()
