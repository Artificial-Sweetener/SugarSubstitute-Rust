#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Fingerprint cube-library sources that contribute dependency requirements."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Protocol

from ...cube_model import RESERVED_SOURCE_NAMES
from .cube_file_io import list_cube_files, safe_relative_path
from .cube_library_catalog_projection import CubeLibraryCatalogProjection
from .cube_library_listing import CubeLibraryListing
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .tracked_repo_service import TrackedRepoService

_SOURCE_SIGNATURE_SCHEMA_VERSION = 1


class DependencyRequirementSourceLibrary(Protocol):
    """Describe library operations needed to fingerprint dependency sources."""

    tracked_repo_service: TrackedRepoService
    catalog_listing: CubeLibraryListing
    catalog_projection: CubeLibraryCatalogProjection
    sources: CubeLibrarySourceResolver


def dependency_requirement_source_signature(
    library: DependencyRequirementSourceLibrary,
) -> str:
    """Return cheap source facts that validate durable requirement reuse."""

    repo_entries = library.tracked_repo_service.list_repos()["repos"]
    repo_cube_facts: list[dict[str, Any]] = []
    for repo_entry in repo_entries:
        if not repo_entry.get("enabled"):
            continue
        tracked = library.catalog_listing.tracked_repo_from_payload(repo_entry)
        repo_cube_facts.extend(
            _requirement_file_facts(
                Path(tracked.local_checkout_path).resolve(),
                source_kind="github",
                owner=tracked.owner,
                repo=tracked.repo,
                namespace="",
            )
        )

    local_cube_facts: list[dict[str, Any]] = []
    local_root = library.sources.local_workspace_root().resolve()
    if local_root.exists():
        for namespace_dir in sorted(
            (path for path in local_root.iterdir() if path.is_dir()),
            key=lambda path: path.name.casefold(),
        ):
            if namespace_dir.name.lower() in RESERVED_SOURCE_NAMES:
                continue
            local_cube_facts.extend(
                _requirement_file_facts(
                    namespace_dir,
                    source_kind="local",
                    owner="",
                    repo="",
                    namespace=namespace_dir.name,
                )
            )
    facts = {
        "schemaVersion": _SOURCE_SIGNATURE_SCHEMA_VERSION,
        "packs": library.catalog_projection.revision_pack_facts(include_disabled=False),
        "repoCubes": repo_cube_facts,
        "localCubes": local_cube_facts,
    }
    serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
    return f"sha256:{hashlib.sha256(serialized.encode('utf-8')).hexdigest()}"


def _requirement_file_facts(
    root: Path,
    *,
    source_kind: str,
    owner: str,
    repo: str,
    namespace: str,
) -> list[dict[str, Any]]:
    """Return stat-only cube facts for a durable requirements cache key."""

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
