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
"""Own dependency discovery, durable caching, and library readiness projections."""

from __future__ import annotations

import hashlib
import json
import logging
import os
from copy import deepcopy
from pathlib import Path
from time import perf_counter
from typing import Any, Mapping, Optional, Protocol, Sequence

from ...cube_model import RESERVED_SOURCE_NAMES
from ...instrumentation import log_diagnostic
from ..responses import BackendError
from .cube_dependency_manifest import (
    iter_custom_node_requirement_ids,
    normalize_requirement_key,
)
from .cube_file_io import list_cube_files, safe_relative_path
from .cube_metadata import normalize_metadata_string
from .dependency_versions import (
    CubeDependencyRequirement,
    classify_version,
    dependency_version_readiness,
    extract_versioned_requirements,
)
from .tracked_repo_service import TrackedRepo, TrackedRepoService

_logger = logging.getLogger(__name__)
_READINESS_TRACE_MARKER = "SugarCubes cube library diagnostic"
_LIBRARY_READINESS_CACHE_TTL_SECONDS = 30.0
_DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION = 1
_DEPENDENCY_REQUIREMENT_CACHE_FILENAME = "dependency-requirements.json"
_DEFAULT_BASE_REPO_REF = "Artificial-Sweetener/Base-Cubes"


class CubeLibraryReadinessOwner(Protocol):
    """Describe the library operations required by readiness projection."""

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

    def _tracked_repo_from_payload(self, repo_entry: Mapping[str, Any]) -> TrackedRepo: ...

    def local_workspace_root(self) -> Path: ...

    def _revision_pack_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]: ...


def _path_mtime_ns(path: Path) -> int:
    """Return a file timestamp suitable for cheap cache invalidation."""

    try:
        return path.stat().st_mtime_ns
    except OSError:
        return 0


def _log_readiness_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured library-readiness diagnostic."""

    log_diagnostic(_logger, _READINESS_TRACE_MARKER, event, fields)


class CubeLibraryReadinessService:
    """Project dependency readiness from one authoritative cube library."""

    def __init__(self, library: CubeLibraryReadinessOwner) -> None:
        """Initialize readiness projection for a library owner."""

        self._library = library
        self._library_readiness_cache: (
            tuple[float, Path, str, dict[str, Any]] | None
        ) = None

    def library_readiness(self, custom_nodes_root: Path) -> dict[str, Any]:
        """Return target dependency readiness and install plan for enabled cubes."""

        started_at = perf_counter()
        phase_started_at = started_at
        phase_timings: dict[str, float] = {}
        custom_nodes_signature = self._library_readiness_cache_signature(
            custom_nodes_root
        )
        cached_payload = self._cached_library_readiness(
            custom_nodes_root=custom_nodes_root,
            custom_nodes_signature=custom_nodes_signature,
        )
        if cached_payload is not None:
            _log_readiness_diagnostic(
                "sugarcubes_library_readiness_cache_hit",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            )
            return cached_payload

        def record_phase(name: str) -> None:
            """Record elapsed milliseconds for one readiness phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round((now - phase_started_at) * 1000, 3)
            phase_started_at = now

        (
            requirement_records,
            version_requirements,
            catalog_revision,
        ) = self._dependency_requirement_sets()
        record_phase("dependency_requirement_sets")
        required = tuple(
            sorted(
                {record["node_id"] for record in requirement_records},
                key=str.casefold,
            )
        )
        installed = self._installed_custom_nodes(custom_nodes_root)
        record_phase("installed_custom_nodes")
        installed_keys = {normalize_requirement_key(slug) for slug in installed}
        missing = tuple(
            slug
            for slug in required
            if normalize_requirement_key(slug) not in installed_keys
        )
        install_plan = self._dependency_install_plan(
            requirement_records=requirement_records,
            installed=installed,
        )
        record_phase("dependency_install_plan")
        installable_missing = [
            item
            for item in install_plan
            if item["installed"] is False and item["installable"] is True
        ]
        version_readiness = dependency_version_readiness(
            requirements=version_requirements,
            custom_nodes_root=custom_nodes_root,
            git_runner=self._library.tracked_repo_service.git_runner,
        )
        record_phase("dependency_version_readiness")
        total_duration_ms = round((perf_counter() - started_at) * 1000, 3)
        _log_readiness_diagnostic(
            "sugarcubes_library_readiness_timing",
            total_duration_ms=total_duration_ms,
            required_count=len(required),
            installed_count=len(installed),
            missing_count=len(missing),
            install_plan_count=len(install_plan),
            version_requirement_count=len(version_requirements),
            **phase_timings,
        )
        payload = {
            "schemaVersion": 1,
            "ready": not missing,
            "requiredCustomNodes": list(required),
            "missingCustomNodes": list(missing),
            "installedCustomNodes": [
                slug
                for slug in required
                if normalize_requirement_key(slug) in installed_keys
            ],
            "canInstall": bool(installable_missing),
            "installSupported": True,
            "catalogRevision": catalog_revision,
            "errors": [
                item["remediation"]
                for item in install_plan
                if item["installed"] is False and item["installable"] is False
            ],
            "installPlan": install_plan,
            "restartRequired": bool(missing),
            **version_readiness,
        }
        self._library_readiness_cache = (
            perf_counter(),
            custom_nodes_root.resolve(),
            custom_nodes_signature,
            deepcopy(payload),
        )
        return payload

    def _cached_library_readiness(
        self,
        *,
        custom_nodes_root: Path,
        custom_nodes_signature: str,
    ) -> dict[str, Any] | None:
        """Return a recent readiness payload when source facts still match."""

        cached = self._library_readiness_cache
        if cached is None:
            return None
        cached_at, cached_root, cached_signature, cached_payload = cached
        if perf_counter() - cached_at > _LIBRARY_READINESS_CACHE_TTL_SECONDS:
            self._library_readiness_cache = None
            return None
        if cached_root != custom_nodes_root.resolve():
            return None
        if cached_signature != custom_nodes_signature:
            self._library_readiness_cache = None
            return None
        return deepcopy(cached_payload)

    def _library_readiness_cache_signature(self, custom_nodes_root: Path) -> str:
        """Return cheap source facts that guard short-lived readiness reuse."""

        custom_node_facts: list[dict[str, Any]] = []
        try:
            entries = sorted(
                (entry for entry in custom_nodes_root.iterdir() if entry.is_dir()),
                key=lambda entry: entry.name.casefold(),
            )
        except OSError as exc:
            custom_node_facts.append(
                {
                    "error": type(exc).__name__,
                    "path": str(custom_nodes_root),
                }
            )
            entries = []
        for entry in entries:
            custom_node_facts.append(
                {
                    "name": entry.name,
                    "path_mtime_ns": _path_mtime_ns(entry),
                    "git_head_mtime_ns": _path_mtime_ns(entry / ".git" / "HEAD"),
                    "git_index_mtime_ns": _path_mtime_ns(entry / ".git" / "index"),
                    "tracking_mtime_ns": _path_mtime_ns(entry / ".tracking"),
                }
            )
        facts = {
            "customNodes": custom_node_facts,
            "dependencySources": self._dependency_requirement_source_signature(),
        }
        serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
        return hashlib.sha256(serialized.encode("utf-8")).hexdigest()

    def _dependency_requirements(self) -> tuple[str, ...]:
        """Return custom-node slugs required by enabled library cubes."""

        required: set[str] = set()
        for record in self._dependency_requirement_records():
            required.add(record["node_id"])
        return tuple(sorted(required))

    def _dependency_requirement_records(self) -> list[dict[str, Any]]:
        """Return custom-node requirements with pack and cube ownership facts."""

        requirement_records, _, _ = self._dependency_requirement_sets()
        return requirement_records

    def _dependency_requirement_sets(
        self,
    ) -> tuple[list[dict[str, Any]], tuple[CubeDependencyRequirement, ...], str]:
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
            """Accumulate elapsed milliseconds for one dependency readiness subphase."""

            phase_timings[name] = round(
                phase_timings[name] + ((perf_counter() - phase_started_at) * 1000),
                3,
            )

        phase_started_at = perf_counter()
        source_signature = self._dependency_requirement_source_signature()
        add_phase_time("source_signature_build", phase_started_at)
        phase_started_at = perf_counter()
        cached = self._cached_dependency_requirement_sets(source_signature)
        add_phase_time("cache_read", phase_started_at)
        if cached is not None:
            requirement_records, version_requirements, catalog_revision = cached
            _log_readiness_diagnostic(
                "sugarcubes_dependency_requirement_sets_timing",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
                cached=True,
                source_signature=source_signature,
                summary_count=0,
                catalog_fact_count=0,
                requirement_record_count=len(requirement_records),
                version_requirement_count=len(version_requirements),
                skipped_payload_count=0,
                repo_lookup_count=0,
                **phase_timings,
            )
            return requirement_records, version_requirements, catalog_revision

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
                payload, error, content_hash = self._library._summary_payload_with_hash(summary)
                add_phase_time("summary_payload_with_hash", phase_started_at)
            except BackendError:
                skipped_payload_count += 1
                continue
            phase_started_at = perf_counter()
            source = self._library._source_metadata_for_summary(summary, repo_cache=repo_cache)
            add_phase_time("source_metadata_for_summary", phase_started_at)
            catalog_facts.append(
                (
                    self._readiness_catalog_sort_key(
                        summary=summary,
                        source=source,
                        cube_id=cube_id,
                    ),
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
            dependency_source = self._dependency_source_for_summary(summary)
            pack_ref = self._dependency_pack_ref(dependency_source)
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
                    source_path=self._dependency_source_path(dependency_source),
                    default_base_repo=default_base_repo,
                )
            )
            add_phase_time("extract_versioned_requirements", phase_started_at)
        phase_started_at = perf_counter()
        catalog_revision = self._readiness_catalog_revision(catalog_facts)
        add_phase_time("readiness_catalog_revision", phase_started_at)
        phase_started_at = perf_counter()
        self._store_dependency_requirement_sets(
            source_signature=source_signature,
            requirement_records=records,
            version_requirements=tuple(version_records),
            catalog_revision=catalog_revision,
        )
        add_phase_time("cache_write", phase_started_at)
        _log_readiness_diagnostic(
            "sugarcubes_dependency_requirement_sets_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            cached=False,
            source_signature=source_signature,
            summary_count=len(summaries),
            catalog_fact_count=len(catalog_facts),
            requirement_record_count=len(records),
            version_requirement_count=len(version_records),
            skipped_payload_count=skipped_payload_count,
            repo_lookup_count=len(repo_cache),
            **phase_timings,
        )
        return records, tuple(version_records), catalog_revision

    def _dependency_requirement_source_signature(self) -> str:
        """Return cheap source facts that validate durable requirement reuse."""

        repo_entries = self._library.tracked_repo_service.list_repos()["repos"]
        repo_cube_facts: list[dict[str, Any]] = []
        for repo_entry in repo_entries:
            if not repo_entry.get("enabled"):
                continue
            tracked = self._library._tracked_repo_from_payload(repo_entry)
            checkout_path = Path(tracked.local_checkout_path).resolve()
            repo_cube_facts.extend(
                self._dependency_requirement_file_facts(
                    checkout_path,
                    source_kind="github",
                    owner=tracked.owner,
                    repo=tracked.repo,
                    namespace="",
                )
            )

        local_cube_facts: list[dict[str, Any]] = []
        local_root = self._library.local_workspace_root().resolve()
        if local_root.exists():
            for namespace_dir in sorted(
                (path for path in local_root.iterdir() if path.is_dir()),
                key=lambda path: path.name.casefold(),
            ):
                namespace = namespace_dir.name
                if namespace.lower() in RESERVED_SOURCE_NAMES:
                    continue
                local_cube_facts.extend(
                    self._dependency_requirement_file_facts(
                        namespace_dir,
                        source_kind="local",
                        owner="",
                        repo="",
                        namespace=namespace,
                    )
                )
        facts = {
            "schemaVersion": _DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION,
            "packs": self._library._revision_pack_facts(include_disabled=False),
            "repoCubes": repo_cube_facts,
            "localCubes": local_cube_facts,
        }
        serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
        return f"sha256:{hashlib.sha256(serialized.encode('utf-8')).hexdigest()}"

    def _dependency_requirement_file_facts(
        self,
        root: Path,
        *,
        source_kind: str,
        owner: str,
        repo: str,
        namespace: str,
    ) -> list[dict[str, Any]]:
        """Return stat-only cube facts for the durable requirements cache key."""

        if not root.exists() or not root.is_dir():
            return []
        facts: list[dict[str, Any]] = []
        for path in list_cube_files(root):
            if ".git" in path.parts:
                continue
            try:
                stat_info = path.stat()
            except OSError as exc:
                facts.append(
                    {
                        "source_kind": source_kind,
                        "owner": owner,
                        "repo": repo,
                        "namespace": namespace,
                        "relative_path": safe_relative_path(path, root) or "",
                        "error": type(exc).__name__,
                    }
                )
                continue
            facts.append(
                {
                    "source_kind": source_kind,
                    "owner": owner,
                    "repo": repo,
                    "namespace": namespace,
                    "relative_path": safe_relative_path(path, root) or "",
                    "size_bytes": stat_info.st_size,
                    "mtime_ns": stat_info.st_mtime_ns,
                }
            )
        return sorted(
            facts,
            key=lambda fact: (
                str(fact.get("source_kind", "")).casefold(),
                str(fact.get("owner", "")).casefold(),
                str(fact.get("repo", "")).casefold(),
                str(fact.get("namespace", "")).casefold(),
                str(fact.get("relative_path", "")).casefold(),
            ),
        )

    def _cached_dependency_requirement_sets(
        self,
        source_signature: str,
    ) -> tuple[list[dict[str, Any]], tuple[CubeDependencyRequirement, ...], str] | None:
        """Return durable dependency requirements when source facts still match."""

        cache_path = self._dependency_requirement_cache_path()
        try:
            raw = json.loads(cache_path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None
        except (OSError, json.JSONDecodeError, UnicodeDecodeError, TypeError):
            _logger.warning(
                "SugarCubes: failed to read dependency requirement cache",
                exc_info=True,
            )
            return None
        if not isinstance(raw, Mapping):
            return None
        if raw.get("schemaVersion") != _DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION:
            return None
        if raw.get("sourceSignature") != source_signature:
            return None
        requirement_records = raw.get("requirementRecords")
        version_requirements = raw.get("versionRequirements")
        catalog_revision = normalize_metadata_string(raw.get("catalogRevision"))
        if not isinstance(requirement_records, list) or not isinstance(
            version_requirements, list
        ):
            return None
        try:
            return (
                [
                    dict(record)
                    for record in requirement_records
                    if isinstance(record, Mapping)
                ],
                tuple(
                    self._dependency_requirement_from_payload(record)
                    for record in version_requirements
                    if isinstance(record, Mapping)
                ),
                catalog_revision,
            )
        except (TypeError, ValueError):
            _logger.warning(
                "SugarCubes: dependency requirement cache payload is invalid",
                exc_info=True,
            )
            return None

    def _store_dependency_requirement_sets(
        self,
        *,
        source_signature: str,
        requirement_records: Sequence[Mapping[str, Any]],
        version_requirements: Sequence[CubeDependencyRequirement],
        catalog_revision: str,
    ) -> None:
        """Persist dependency requirements for reuse by the next Comfy process."""

        cache_path = self._dependency_requirement_cache_path()
        payload = {
            "schemaVersion": _DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION,
            "sourceSignature": source_signature,
            "catalogRevision": catalog_revision,
            "requirementRecords": [dict(record) for record in requirement_records],
            "versionRequirements": [
                requirement.to_payload() for requirement in version_requirements
            ],
        }
        try:
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            temp_path = cache_path.with_name(f"{cache_path.name}.{os.getpid()}.tmp")
            temp_path.write_text(
                json.dumps(payload, sort_keys=True, separators=(",", ":")),
                encoding="utf-8",
            )
            temp_path.replace(cache_path)
        except OSError:
            _logger.warning(
                "SugarCubes: failed to write dependency requirement cache",
                exc_info=True,
            )

    def _dependency_requirement_cache_path(self) -> Path:
        """Return the durable dependency-requirement cache location."""

        return (
            self._library.extension_root
            / ".sugarcubes"
            / "cache"
            / _DEPENDENCY_REQUIREMENT_CACHE_FILENAME
        )

    def _dependency_requirement_from_payload(
        self,
        payload: Mapping[str, Any],
    ) -> CubeDependencyRequirement:
        """Rehydrate one cached versioned dependency requirement."""

        return CubeDependencyRequirement(
            node_id=normalize_metadata_string(payload.get("nodeId")),
            required_version=normalize_metadata_string(payload.get("requiredVersion")),
            version_kind=classify_version(
                normalize_metadata_string(payload.get("requiredVersion"))
            ),
            cube_id=normalize_metadata_string(payload.get("cubeId")),
            pack_ref=normalize_metadata_string(payload.get("packRef")),
            node_name=normalize_metadata_string(payload.get("nodeName")),
            class_type=normalize_metadata_string(payload.get("classType")),
            source_path=normalize_metadata_string(payload.get("sourcePath")),
            default_base_repo=bool(payload.get("defaultBaseRepo")),
        )

    def _readiness_catalog_sort_key(
        self,
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

    def _readiness_catalog_revision(
        self,
        catalog_facts: Sequence[tuple[tuple[str, str, str, str, str], dict[str, Any]]],
    ) -> str:
        """Return the catalog revision from readiness' already-read cube facts."""

        facts = {
            "packs": self._library._revision_pack_facts(include_disabled=False),
            "cubes": [
                fact for _, fact in sorted(catalog_facts, key=lambda item: item[0])
            ],
        }
        serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
        return f"sha256:{hashlib.sha256(serialized.encode('utf-8')).hexdigest()}"

    def _dependency_source_for_summary(
        self,
        summary: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Return cheap source facts needed by dependency readiness."""

        source_value = summary.get("source")
        source = source_value if isinstance(source_value, Mapping) else {}
        source_kind = normalize_metadata_string(
            source.get("type")
        ) or normalize_metadata_string(summary.get("source_kind"))
        if source_kind == "github":
            owner = normalize_metadata_string(
                source.get("owner") or summary.get("owner")
            )
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

    def _dependency_version_requirement_records(
        self,
    ) -> tuple[CubeDependencyRequirement, ...]:
        """Return version-aware dependency facts with cube ownership context."""

        _, version_records, _ = self._dependency_requirement_sets()
        return version_records

    def _dependency_install_plan(
        self,
        *,
        requirement_records: Sequence[Mapping[str, Any]],
        installed: set[str],
    ) -> list[dict[str, Any]]:
        """Collapse requirement records into one install plan per custom node."""

        installed_by_key = {
            normalize_requirement_key(name): name for name in installed
        }
        by_node: dict[str, dict[str, Any]] = {}
        for record in requirement_records:
            node_id = normalize_metadata_string(record.get("node_id"))
            if not node_id:
                continue
            key = normalize_requirement_key(node_id)
            item = by_node.setdefault(
                key,
                {
                    "nodeId": node_id,
                    "displayName": normalize_metadata_string(record.get("display_name"))
                    or node_id,
                    "existingFolderName": "",
                    "requiredByPacks": [],
                    "requiredByCubeIds": [],
                    "defaultBaseOnly": True,
                    "confirmationRequired": False,
                    "installable": True,
                    "installed": False,
                    "remediation": "",
                },
            )
            pack_ref = normalize_metadata_string(record.get("pack_ref"))
            cube_id = normalize_metadata_string(record.get("cube_id"))
            if pack_ref and pack_ref not in item["requiredByPacks"]:
                item["requiredByPacks"].append(pack_ref)
            if cube_id and cube_id not in item["requiredByCubeIds"]:
                item["requiredByCubeIds"].append(cube_id)
            if not bool(record.get("default_base_repo")):
                item["defaultBaseOnly"] = False
                item["confirmationRequired"] = True

        for key, item in by_node.items():
            existing_folder = installed_by_key.get(key, "")
            item["existingFolderName"] = existing_folder
            item["installed"] = bool(existing_folder)
            item["requiredByPacks"].sort(key=str.casefold)
            item["requiredByCubeIds"].sort(key=str.casefold)
            if not item["installed"] and not item["nodeId"]:
                item["installable"] = False
                item["remediation"] = (
                    "Cube requirement does not include a Comfy Registry id."
                )
        return sorted(by_node.values(), key=lambda item: str(item["nodeId"]).casefold())

    def _dependency_pack_ref(self, source: Mapping[str, Any]) -> str:
        """Return a stable source label for dependency prompt grouping."""

        source_kind = normalize_metadata_string(source.get("kind"))
        if source_kind == "github":
            return normalize_metadata_string(source.get("repoRef"))
        namespace = normalize_metadata_string(source.get("namespace"))
        return f"local/{namespace}" if namespace else "local"

    def _dependency_source_path(self, source: Mapping[str, Any]) -> str:
        """Return a non-absolute source path for dependency diagnostics."""

        source_kind = normalize_metadata_string(source.get("kind"))
        if source_kind == "github":
            repo_ref = normalize_metadata_string(source.get("repoRef"))
            path = normalize_metadata_string(source.get("path"))
            return f"{repo_ref}/{path}" if path else repo_ref
        namespace = normalize_metadata_string(source.get("namespace"))
        path = normalize_metadata_string(source.get("path"))
        return f"local/{namespace}/{path}".rstrip("/") if namespace else "local"

    def _installed_custom_nodes(self, custom_nodes_root: Path) -> set[str]:
        """Return installed target custom-node directory names."""

        if not custom_nodes_root.exists() or not custom_nodes_root.is_dir():
            return set()
        return {
            entry.name
            for entry in custom_nodes_root.iterdir()
            if entry.is_dir() and entry.name
        }

