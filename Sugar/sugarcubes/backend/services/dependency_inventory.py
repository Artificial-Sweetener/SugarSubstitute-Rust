#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Inspect installed custom-node dependency evidence."""

from __future__ import annotations

import json
import logging
import tomllib
from collections.abc import Callable, Collection, Mapping
from pathlib import Path
from time import perf_counter
from typing import Any

from ...instrumentation import log_diagnostic
from .dependency_requirements import normalize_requirement_key
from .dependency_version_types import InstalledDependency
from .dependency_versions import classify_version
from .repository_service import RepositoryOperationError, RepositoryService

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube library diagnostic"
_SLOW_INVENTORY_ENTRY_MS = 250.0


def installed_dependency_inventory(
    custom_nodes_root: Path,
    *,
    repositories: RepositoryService,
    detailed_keys: Collection[str] | None = None,
) -> dict[str, InstalledDependency]:
    """Inspect installed custom-node folders without mutating them."""

    started_at = perf_counter()
    if not custom_nodes_root.exists() or not custom_nodes_root.is_dir():
        return {}
    inventory: dict[str, InstalledDependency] = {}
    slow_entries: list[dict[str, Any]] = []
    phase_timings = {
        "list_custom_node_entries": 0.0,
        "read_tracking_metadata": 0.0,
        "probe_git_dir": 0.0,
        "read_git_head": 0.0,
        "read_git_status": 0.0,
        "read_git_remote": 0.0,
    }
    list_started_at = perf_counter()
    entries = tuple(custom_nodes_root.iterdir())
    phase_timings["list_custom_node_entries"] = round(
        (perf_counter() - list_started_at) * 1000,
        3,
    )
    detailed_git_count = 0
    cheap_git_count = 0
    for entry in entries:
        if not entry.is_dir() or not entry.name:
            continue
        entry_started_at = perf_counter()
        detailed_git = (
            detailed_keys is None
            or normalize_requirement_key(entry.name) in detailed_keys
        )
        dependency = _installed_dependency(
            entry,
            repositories=repositories,
            detailed_git=detailed_git,
            phase_timings=phase_timings,
        )
        if dependency.source_kind == "git":
            if detailed_git:
                detailed_git_count += 1
            else:
                cheap_git_count += 1
        entry_duration_ms = round((perf_counter() - entry_started_at) * 1000, 3)
        inventory[entry.name] = dependency
        if entry_duration_ms >= _SLOW_INVENTORY_ENTRY_MS:
            slow_entries.append(
                {
                    "folder": entry.name,
                    "duration_ms": entry_duration_ms,
                    "source_kind": dependency.source_kind,
                }
            )
    log_diagnostic(
        _logger,
        _TRACE_MARKER,
        "sugarcubes_installed_dependency_inventory_timing",
        {
            "total_duration_ms": round((perf_counter() - started_at) * 1000, 3),
            "entry_count": len(inventory),
            "git_entry_count": sum(
                1 for item in inventory.values() if item.source_kind == "git"
            ),
            "tracking_entry_count": sum(
                1 for item in inventory.values() if item.source_kind == "tracking"
            ),
            "directory_entry_count": sum(
                1 for item in inventory.values() if item.source_kind == "directory"
            ),
            "detailed_git_count": detailed_git_count,
            "cheap_git_count": cheap_git_count,
            "slow_entries": slow_entries,
            **phase_timings,
        },
    )
    return inventory


def _installed_dependency(
    path: Path,
    *,
    repositories: RepositoryService,
    detailed_git: bool,
    phase_timings: dict[str, float],
) -> InstalledDependency:
    """Inspect one installed custom-node folder."""

    git_dir = path / ".git"
    phase_started_at = perf_counter()
    tracking_path = path / ".tracking"
    tracking = _read_tracking_metadata(tracking_path)
    project_version, project_repository = _read_project_identity(
        path / "pyproject.toml"
    )
    _add_phase_time(phase_timings, "read_tracking_metadata", phase_started_at)
    phase_started_at = perf_counter()
    git_exists = git_dir.exists()
    _add_phase_time(phase_timings, "probe_git_dir", phase_started_at)
    if git_exists:
        if not detailed_git:
            return _cheap_git_dependency(
                path,
                tracking,
                phase_timings=phase_timings,
            )
        phase_started_at = perf_counter()
        head = _repository_value(repositories.head_commit_id, path)
        _add_phase_time(phase_timings, "read_git_head", phase_started_at)
        phase_started_at = perf_counter()
        dirty = _repository_dirty(repositories, path)
        _add_phase_time(phase_timings, "read_git_status", phase_started_at)
        phase_started_at = perf_counter()
        repository_url = _repository_value(repositories.remote_url, path)
        _add_phase_time(phase_timings, "read_git_remote", phase_started_at)
        return InstalledDependency(
            folder_name=path.name,
            source_path=str(path),
            installed_version=project_version or head,
            version_kind=classify_version(project_version or head),
            source_kind="git",
            repository_url=repository_url,
            dirty=dirty,
            git_head=head,
            project_version=project_version,
        )
    version = project_version or _normalize_text(tracking.get("version"))
    return InstalledDependency(
        folder_name=path.name,
        source_path=str(path),
        installed_version=version,
        version_kind=classify_version(version),
        source_kind="tracking" if tracking_path.is_file() else "directory",
        repository_url=(
            project_repository or _normalize_text(tracking.get("repository"))
        ),
        dirty=False,
        project_version=project_version,
    )


def _cheap_git_dependency(
    path: Path,
    tracking: Mapping[str, Any],
    *,
    phase_timings: dict[str, float],
) -> InstalledDependency:
    """Return Git evidence for non-required nodes without subprocess probes."""

    phase_started_at = perf_counter()
    head = _read_git_head(path / ".git")
    _add_phase_time(phase_timings, "read_git_head", phase_started_at)
    return InstalledDependency(
        folder_name=path.name,
        source_path=str(path),
        installed_version=head,
        version_kind=classify_version(head),
        source_kind="git",
        repository_url=_normalize_text(tracking.get("repository")),
        dirty=False,
        git_head=head,
    )


def _add_phase_time(
    phase_timings: dict[str, float],
    name: str,
    started_at: float,
) -> None:
    """Accumulate elapsed milliseconds for one inventory subphase."""

    phase_timings[name] = round(
        phase_timings[name] + ((perf_counter() - started_at) * 1000),
        3,
    )


def _read_git_head(git_dir: Path) -> str:
    """Read a Git HEAD value directly for non-authoritative evidence."""

    resolved_git_dir = _resolve_git_dir(git_dir)
    if resolved_git_dir is None:
        return ""
    head = _read_text_file(resolved_git_dir / "HEAD")
    if head.startswith("ref:"):
        ref_name = head.removeprefix("ref:").strip()
        return _read_text_file(resolved_git_dir / ref_name)
    return head


def _resolve_git_dir(git_path: Path) -> Path | None:
    """Return the concrete Git metadata directory for repos and worktrees."""

    if git_path.is_dir():
        return git_path
    if not git_path.is_file():
        return None
    text = _read_text_file(git_path)
    if not text.startswith("gitdir:"):
        return None
    target = Path(text.removeprefix("gitdir:").strip())
    if not target.is_absolute():
        target = git_path.parent / target
    return target


def _read_text_file(path: Path) -> str:
    """Return stripped UTF-8 text from one small metadata file."""

    try:
        return path.read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def _read_tracking_metadata(path: Path) -> dict[str, Any]:
    """Read best-effort Comfy Manager tracking metadata."""

    if not path.exists() or not path.is_file():
        return {}
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return {}
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        return {"raw": text}
    return dict(data) if isinstance(data, Mapping) else {}


def _read_project_identity(path: Path) -> tuple[str, str]:
    """Read Registry project version and repository from installed source."""

    if not path.is_file():
        return "", ""
    try:
        payload = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return "", ""
    project = payload.get("project")
    if not isinstance(project, Mapping):
        return "", ""
    version = _normalize_text(project.get("version"))
    urls = project.get("urls")
    repository = (
        _normalize_text(urls.get("Repository")) if isinstance(urls, Mapping) else ""
    )
    return version, repository


def _repository_value(
    operation: Callable[[Path], str],
    repository_path: Path,
) -> str:
    """Return one repository string while treating unreadable evidence as absent."""

    try:
        value = operation(repository_path)
    except (OSError, RepositoryOperationError, ValueError):
        return ""
    return _normalize_text(value)


def _repository_dirty(
    repositories: RepositoryService,
    repository_path: Path,
) -> bool:
    """Return dirty state while treating unreadable evidence as clean."""

    try:
        return repositories.is_dirty(repository_path)
    except (OSError, RepositoryOperationError, ValueError):
        return False


def _normalize_text(value: object) -> str:
    """Return a stripped string or an empty string."""

    return value.strip() if isinstance(value, str) else ""
