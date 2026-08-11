#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Discover and durably cache dependency requirements from the cube library."""

from __future__ import annotations

import hashlib
import json
import logging
from collections.abc import Mapping, Sequence
from pathlib import Path
from time import perf_counter
from typing import Any, Optional, Protocol

from ...instrumentation import log_diagnostic
from ..responses import BackendError
from .cube_dependency_manifest import iter_custom_node_requirement_ids
from .cube_metadata import normalize_metadata_string
from .dependency_requirement_cache import (
    DependencyRequirementCache,
    DependencyRequirementSet,
)
from .dependency_requirement_sources import (
    DependencyRequirementSourceLibrary,
    dependency_requirement_source_signature,
)
from .dependency_requirements import extract_versioned_requirements
from .dependency_version_types import CubeDependencyRequirement
from .tracked_repo_models import TrackedRepo
from .tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube library diagnostic"
_DEFAULT_BASE_REPO_REF = "Artificial-Sweetener/Base-Cubes"


class DependencyRequirementLibrary(DependencyRequirementSourceLibrary, Protocol):
    """Describe library operations needed for requirement discovery."""

    extension_root: Path
    tracked_repo_service: TrackedRepoService

    def _list_catalog_cube_summaries(
        self,
        *,
        include_disabled: bool,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]: ...

    def _summary_payload_with_hash(
        self, summary: Mapping[str, Any]
    ) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]: ...

    def _source_metadata_for_summary(
        self,
        summary: Mapping[str, Any],
        *,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None = None,
    ) -> dict[str, Any]: ...


class DependencyRequirementInventory:
    """Discover, fingerprint, and cache dependency requirements."""

    def __init__(self, library: DependencyRequirementLibrary) -> None:
        """Initialize inventory for one authoritative cube library."""

        self._library = library
        self._cache = DependencyRequirementCache(library.extension_root)

    def collect(self) -> DependencyRequirementSet:
        """Return dependency requirements and revision facts from one cube pass."""

        started_at = perf_counter()
        phase_timings = {
            "source_signature_build": 0.0,
            "cache_read": 0.0,
            "list_catalog_cube_summaries": 0.0,
            "summary_payload_with_hash": 0.0,
            "source_metadata_for_summary": 0.0,
            "iter_custom_node_requirement_ids": 0.0,
            "extract_versioned_requirements": 0.0,
            "readiness_catalog_revision": 0.0,
            "cache_write": 0.0,
        }

        def add_phase_time(name: str, phase_started_at: float) -> None:
            """Accumulate elapsed milliseconds for one inventory subphase."""

            phase_timings[name] = round(
                phase_timings[name] + ((perf_counter() - phase_started_at) * 1000),
                3,
            )

        phase_started_at = perf_counter()
        source_signature = self.source_signature()
        add_phase_time("source_signature_build", phase_started_at)
        phase_started_at = perf_counter()
        cached = self._cache.load(source_signature)
        add_phase_time("cache_read", phase_started_at)
        if cached is not None:
            self._log_collection(
                started_at=started_at,
                phase_timings=phase_timings,
                source_signature=source_signature,
                result=cached,
                cached=True,
            )
            return cached

        records: list[dict[str, Any]] = []
        version_records: list[CubeDependencyRequirement] = []
        catalog_facts: list[tuple[tuple[str, str, str, str, str], dict[str, Any]]] = []
        repo_cache: dict[tuple[str, str], TrackedRepo] = {}
        phase_started_at = perf_counter()
        summaries = self._library._list_catalog_cube_summaries(
            include_disabled=False,
            include_internal_payload=True,
        )
        add_phase_time("list_catalog_cube_summaries", phase_started_at)
        skipped_payload_count = 0
        for summary in summaries:
            cube_id = normalize_metadata_string(summary.get("cube_id"))
            try:
                phase_started_at = perf_counter()
                payload, error, content_hash = self._library._summary_payload_with_hash(
                    summary
                )
                add_phase_time("summary_payload_with_hash", phase_started_at)
            except BackendError:
                skipped_payload_count += 1
                continue
            phase_started_at = perf_counter()
            source = self._library._source_metadata_for_summary(
                summary, repo_cache=repo_cache
            )
            add_phase_time("source_metadata_for_summary", phase_started_at)
            catalog_facts.append(
                (
                    _catalog_sort_key(summary=summary, source=source, cube_id=cube_id),
                    {
                        "cube_id": cube_id,
                        "version": normalize_metadata_string(summary.get("version")),
                        "content_hash": content_hash,
                        "source": source,
                    },
                )
            )
            if error or not payload:
                continue
            dependency_source = _dependency_source_for_summary(summary)
            pack_ref = _dependency_pack_ref(dependency_source)
            default_base_repo = pack_ref == _DEFAULT_BASE_REPO_REF
            phase_started_at = perf_counter()
            for node_id in iter_custom_node_requirement_ids(payload):
                records.append(
                    {
                        "node_id": node_id,
                        "display_name": node_id,
                        "pack_ref": pack_ref,
                        "cube_id": cube_id,
                        "default_base_repo": default_base_repo,
                    }
                )
            add_phase_time("iter_custom_node_requirement_ids", phase_started_at)
            phase_started_at = perf_counter()
            version_records.extend(
                extract_versioned_requirements(
                    payload,
                    cube_id=cube_id,
                    pack_ref=pack_ref,
                    source_path=_dependency_source_path(dependency_source),
                    default_base_repo=default_base_repo,
                )
            )
            add_phase_time("extract_versioned_requirements", phase_started_at)
        phase_started_at = perf_counter()
        result = DependencyRequirementSet(
            records=records,
            version_requirements=tuple(version_records),
            catalog_revision=self._catalog_revision(catalog_facts),
        )
        add_phase_time("readiness_catalog_revision", phase_started_at)
        phase_started_at = perf_counter()
        self._cache.store(source_signature=source_signature, result=result)
        add_phase_time("cache_write", phase_started_at)
        self._log_collection(
            started_at=started_at,
            phase_timings=phase_timings,
            source_signature=source_signature,
            result=result,
            cached=False,
            summary_count=len(summaries),
            catalog_fact_count=len(catalog_facts),
            skipped_payload_count=skipped_payload_count,
            repo_lookup_count=len(repo_cache),
        )
        return result

    def source_signature(self) -> str:
        """Return cheap source facts that validate durable requirement reuse."""

        return dependency_requirement_source_signature(self._library)

    def _catalog_revision(
        self,
        catalog_facts: Sequence[tuple[tuple[str, str, str, str, str], dict[str, Any]]],
    ) -> str:
        """Return the catalog revision from already-read cube facts."""

        facts = {
            "packs": self._library._revision_pack_facts(include_disabled=False),
            "cubes": [
                fact for _, fact in sorted(catalog_facts, key=lambda item: item[0])
            ],
        }
        serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
        return f"sha256:{hashlib.sha256(serialized.encode('utf-8')).hexdigest()}"

    def _log_collection(
        self,
        *,
        started_at: float,
        phase_timings: Mapping[str, float],
        source_signature: str,
        result: DependencyRequirementSet,
        cached: bool,
        summary_count: int = 0,
        catalog_fact_count: int = 0,
        skipped_payload_count: int = 0,
        repo_lookup_count: int = 0,
    ) -> None:
        """Emit requirement collection timing and result counts."""

        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_dependency_requirement_sets_timing",
            {
                "total_duration_ms": round((perf_counter() - started_at) * 1000, 3),
                "cached": cached,
                "source_signature": source_signature,
                "summary_count": summary_count,
                "catalog_fact_count": catalog_fact_count,
                "requirement_record_count": len(result.records),
                "version_requirement_count": len(result.version_requirements),
                "skipped_payload_count": skipped_payload_count,
                "repo_lookup_count": repo_lookup_count,
                **phase_timings,
            },
        )


def _catalog_sort_key(
    *,
    summary: Mapping[str, Any],
    source: Mapping[str, Any],
    cube_id: str,
) -> tuple[str, str, str, str, str]:
    """Return the catalog ordering used by readiness revision facts."""

    return (
        str(source.get("kind", "")).casefold(),
        str(source.get("repoRef", "")).casefold(),
        normalize_metadata_string(summary.get("target_model")).casefold(),
        (
            normalize_metadata_string(summary.get("display_name"))
            or normalize_metadata_string(summary.get("name"))
        ).casefold(),
        cube_id.casefold(),
    )


def _dependency_source_for_summary(summary: Mapping[str, Any]) -> dict[str, Any]:
    """Return cheap source facts needed by dependency readiness."""

    source_value = summary.get("source")
    source = source_value if isinstance(source_value, Mapping) else {}
    source_kind = normalize_metadata_string(
        source.get("type")
    ) or normalize_metadata_string(summary.get("source_kind"))
    if source_kind == "github":
        owner = normalize_metadata_string(source.get("owner") or summary.get("owner"))
        repo = normalize_metadata_string(source.get("repo") or summary.get("repo"))
        relative_path = normalize_metadata_string(
            source.get("repo_relative_path") or summary.get("relative_path")
        )
        return {
            "kind": "github",
            "repoRef": f"{owner}/{repo}",
            "owner": owner,
            "repo": repo,
            "path": relative_path,
        }
    namespace = normalize_metadata_string(
        source.get("namespace") or summary.get("namespace")
    )
    return {
        "kind": "local",
        "namespace": namespace,
        "path": normalize_metadata_string(summary.get("relative_path")),
    }


def _dependency_pack_ref(source: Mapping[str, Any]) -> str:
    """Return a stable source label for dependency prompt grouping."""

    if normalize_metadata_string(source.get("kind")) == "github":
        return normalize_metadata_string(source.get("repoRef"))
    namespace = normalize_metadata_string(source.get("namespace"))
    return f"local/{namespace}" if namespace else "local"


def _dependency_source_path(source: Mapping[str, Any]) -> str:
    """Return a non-absolute source path for dependency diagnostics."""

    if normalize_metadata_string(source.get("kind")) == "github":
        repo_ref = normalize_metadata_string(source.get("repoRef"))
        path = normalize_metadata_string(source.get("path"))
        return f"{repo_ref}/{path}" if path else repo_ref
    namespace = normalize_metadata_string(source.get("namespace"))
    path = normalize_metadata_string(source.get("path"))
    return f"local/{namespace}/{path}".rstrip("/") if namespace else "local"
