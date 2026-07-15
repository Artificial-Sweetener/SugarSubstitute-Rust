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
"""Versioned cube dependency extraction and readiness planning."""

from __future__ import annotations

import json
import logging
import re
from collections import defaultdict
from collections.abc import Callable, Collection, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from time import perf_counter
from typing import Any, Literal

from ...instrumentation import log_diagnostic

_logger = logging.getLogger(__name__)
CUBE_LIBRARY_TRACE_MARKER = "SugarCubes cube library diagnostic"
_SLOW_INVENTORY_ENTRY_MS = 250.0

VersionKind = Literal["semver", "git_sha", "unknown", "missing"]
DependencyStatus = Literal[
    "satisfied",
    "missing",
    "installed_version_unknown",
    "installed_version_too_old",
    "installed_commit_not_descendant",
    "version_conflict",
    "not_comparable",
    "not_repairable",
    "blocked",
]
GitRunner = Callable[..., object]
GitContainsCache = dict[tuple[str, str, str], bool]

_SEMVER_RE = re.compile(r"^\d+(?:\.\d+){1,3}(?:[-+][0-9A-Za-z.-]+)?$")
_GIT_SHA_RE = re.compile(r"^[0-9a-fA-F]{7,40}$")
_EXCLUDED_CUSTOM_NODE_SLUGS = frozenset({"websocket_image_save"})
_BUILT_IN_CUSTOM_NODE_IDS = frozenset({"comfy-core"})
_SUGARCUBES_CUSTOM_NODE_IDS = frozenset({"sugarcubes"})
_SUGARCUBES_MARKER_MODULES = frozenset({"nodes", "payloads"})


@dataclass(frozen=True)
class CubeDependencyRequirement:
    """Describe one dependency fact contributed by one cube node or fallback module."""

    node_id: str
    required_version: str
    version_kind: VersionKind
    cube_id: str
    pack_ref: str
    node_name: str
    class_type: str
    source_path: str
    default_base_repo: bool

    def to_payload(self) -> dict[str, Any]:
        """Return this requirement as a JSON-safe payload."""

        return {
            "nodeId": self.node_id,
            "requiredVersion": self.required_version,
            "requiredVersionKind": self.version_kind,
            "cubeId": self.cube_id,
            "packRef": self.pack_ref,
            "nodeName": self.node_name,
            "classType": self.class_type,
            "sourcePath": self.source_path,
            "defaultBaseRepo": self.default_base_repo,
        }


@dataclass(frozen=True)
class InstalledDependency:
    """Describe installed evidence for one custom-node folder."""

    folder_name: str
    source_path: str
    installed_version: str
    version_kind: VersionKind
    source_kind: str
    repository_url: str
    dirty: bool

    def to_payload(self) -> dict[str, Any]:
        """Return this installed evidence as a JSON-safe payload."""

        return {
            "folderName": self.folder_name,
            "sourcePath": self.source_path,
            "installedVersion": self.installed_version,
            "installedVersionKind": self.version_kind,
            "sourceKind": self.source_kind,
            "repositoryUrl": self.repository_url,
            "dirty": self.dirty,
        }


def extract_versioned_requirements(
    payload: Mapping[str, Any],
    *,
    cube_id: str,
    pack_ref: str,
    source_path: str,
    default_base_repo: bool,
) -> tuple[CubeDependencyRequirement, ...]:
    """Extract versioned dependency records from current and legacy cube payloads."""

    records: list[CubeDependencyRequirement] = []
    for node in _iter_workflow_nodes(payload):
        properties = node.get("properties")
        if not isinstance(properties, Mapping):
            continue
        node_id = _normalize_text(properties.get("cnr_id"))
        if not node_id:
            continue
        required_version = _normalize_text(properties.get("ver"))
        records.append(
            CubeDependencyRequirement(
                node_id=node_id,
                required_version=required_version,
                version_kind=classify_version(required_version),
                cube_id=cube_id,
                pack_ref=pack_ref,
                node_name=_normalize_text(properties.get("Node name for S&R"))
                or _normalize_text(node.get("type"))
                or node_id,
                class_type=_normalize_text(node.get("type")),
                source_path=source_path,
                default_base_repo=default_base_repo,
            )
        )

    existing_keys = {_requirement_key(record.node_id) for record in records}
    for slug in _iter_custom_node_slugs(payload):
        if not _is_external_custom_node_requirement(slug):
            continue
        if _requirement_key(slug) in existing_keys:
            continue
        records.append(
            CubeDependencyRequirement(
                node_id=slug,
                required_version="",
                version_kind="missing",
                cube_id=cube_id,
                pack_ref=pack_ref,
                node_name=slug,
                class_type="",
                source_path=source_path,
                default_base_repo=default_base_repo,
            )
        )
    return tuple(records)


def dependency_version_readiness(
    *,
    requirements: Sequence[CubeDependencyRequirement],
    custom_nodes_root: Path,
    git_runner: GitRunner | None,
) -> dict[str, Any]:
    """Build an additive version-readiness payload from cube requirements."""

    started_at = perf_counter()
    phase_started_at = started_at
    phase_timings: dict[str, float] = {}

    def record_phase(name: str) -> None:
        """Record elapsed milliseconds for one version-readiness phase."""

        nonlocal phase_started_at
        now = perf_counter()
        phase_timings[name] = round((now - phase_started_at) * 1000, 3)
        phase_started_at = now

    custom_node_requirements = [
        requirement
        for requirement in requirements
        if _is_external_custom_node_requirement(requirement.node_id)
    ]
    record_phase("filter_custom_node_requirements")
    grouped: dict[str, list[CubeDependencyRequirement]] = defaultdict(list)
    for requirement in custom_node_requirements:
        grouped[_requirement_key(requirement.node_id)].append(requirement)
    record_phase("group_requirements")
    git_contains_cache: GitContainsCache = {}
    installed = installed_dependency_inventory(
        custom_nodes_root,
        git_runner=git_runner,
        detailed_keys=frozenset(grouped),
    )
    record_phase("installed_dependency_inventory")
    installed_by_key = {
        _requirement_key(item.folder_name): item for item in installed.values()
    }

    plan = [
        _version_plan_item(
            node_requirements=node_requirements,
            installed=installed_by_key.get(key),
            git_runner=git_runner,
            git_contains_cache=git_contains_cache,
        )
        for key, node_requirements in sorted(
            grouped.items(),
            key=lambda item: item[0],
        )
    ]
    record_phase("build_version_plan")
    payload = {
        "versionedRequirementsSupported": True,
        "dependencyRequirements": [
            requirement.to_payload() for requirement in custom_node_requirements
        ],
        "installedDependencyEvidence": [
            item.to_payload()
            for item in sorted(
                installed.values(),
                key=lambda item: item.folder_name.casefold(),
            )
        ],
        "dependencyVersionPlan": plan,
        "comfyRuntimeReadiness": comfy_runtime_readiness(requirements),
    }
    record_phase("build_payload")
    log_diagnostic(
        _logger,
        CUBE_LIBRARY_TRACE_MARKER,
        "sugarcubes_dependency_version_readiness_timing",
        {
            "total_duration_ms": round((perf_counter() - started_at) * 1000, 3),
            "requirement_count": len(requirements),
            "custom_node_requirement_count": len(custom_node_requirements),
            "installed_count": len(installed),
            "group_count": len(grouped),
            "plan_count": len(plan),
            "git_contains_check_count": len(git_contains_cache),
            **phase_timings,
        },
    )
    return payload


def installed_dependency_inventory(
    custom_nodes_root: Path,
    *,
    git_runner: GitRunner | None,
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
            detailed_keys is None or _requirement_key(entry.name) in detailed_keys
        )
        dependency = _installed_dependency(
            entry,
            git_runner=git_runner,
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
        CUBE_LIBRARY_TRACE_MARKER,
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


def comfy_runtime_readiness(
    requirements: Sequence[CubeDependencyRequirement],
) -> dict[str, Any]:
    """Return Comfy runtime requirements represented by `comfy-core` cube facts."""

    core_requirements = [
        requirement
        for requirement in requirements
        if _requirement_key(requirement.node_id) == "comfy-core"
    ]
    if not core_requirements:
        return {
            "schemaVersion": 1,
            "requiredVersion": "",
            "requiredVersionKind": "missing",
            "installedVersion": "",
            "status": "satisfied",
            "requirements": [],
        }
    strongest = _strongest_semver_requirement(core_requirements)
    return {
        "schemaVersion": 1,
        "requiredVersion": strongest,
        "requiredVersionKind": classify_version(strongest),
        "installedVersion": "",
        "status": "installed_version_unknown",
        "requirements": [requirement.to_payload() for requirement in core_requirements],
        "remediation": "Comfy runtime version could not be read from the active host.",
    }


def classify_version(value: str) -> VersionKind:
    """Classify a cube `ver` token without guessing arbitrary ordering."""

    normalized = _normalize_text(value)
    if not normalized:
        return "missing"
    if _GIT_SHA_RE.fullmatch(normalized):
        return "git_sha"
    if _SEMVER_RE.fullmatch(normalized):
        return "semver"
    return "unknown"


def _version_plan_item(
    *,
    node_requirements: Sequence[CubeDependencyRequirement],
    installed: InstalledDependency | None,
    git_runner: GitRunner | None,
    git_contains_cache: GitContainsCache,
) -> dict[str, Any]:
    """Collapse one node's requirements and installed state into a plan item."""

    node_id = node_requirements[0].node_id
    kinds = {
        requirement.version_kind
        for requirement in node_requirements
        if requirement.version_kind != "missing"
    }
    required_version = _required_version(node_requirements)
    required_kind = classify_version(required_version)
    conflicts = _requirement_conflicts(
        node_requirements=node_requirements,
        installed=installed,
        git_runner=git_runner,
        git_contains_cache=git_contains_cache,
    )
    if installed is None:
        status: DependencyStatus = "missing"
        repairable = True
        installed_version = ""
        installed_kind: VersionKind = "missing"
        remediation = "Install the required custom node."
    elif conflicts:
        status = "version_conflict"
        repairable = False
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = "Resolve conflicting cube version requirements before repair."
    elif not kinds:
        status = "satisfied"
        repairable = False
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = ""
    elif required_kind == "semver":
        status = _semver_status(required_version, installed)
        repairable = status != "satisfied"
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = _remediation_for_status(status)
    elif required_kind == "git_sha":
        status = _git_status(
            required_version,
            installed,
            git_runner=git_runner,
            git_contains_cache=git_contains_cache,
        )
        repairable = status in {"installed_commit_not_descendant"}
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = _remediation_for_status(status)
    else:
        status = "not_comparable"
        repairable = False
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = "Cube requirement version is not comparable."

    return {
        "nodeId": node_id,
        "displayName": node_id,
        "requiredVersion": required_version,
        "requiredVersionKind": required_kind,
        "installedVersion": installed_version,
        "installedVersionKind": installed_kind,
        "installedEvidence": installed.to_payload() if installed is not None else None,
        "status": status,
        "repairable": repairable,
        "restartRequiredAfterRepair": status != "satisfied" and repairable,
        "requiredByPacks": _unique_sorted(
            requirement.pack_ref for requirement in node_requirements
        ),
        "requiredByCubeIds": _unique_sorted(
            requirement.cube_id for requirement in node_requirements
        ),
        "requiredByNodes": _unique_sorted(
            requirement.node_name for requirement in node_requirements
        ),
        "requirements": [requirement.to_payload() for requirement in node_requirements],
        "conflicts": conflicts,
        "remediation": remediation,
    }


def _requirement_conflicts(
    *,
    node_requirements: Sequence[CubeDependencyRequirement],
    installed: InstalledDependency | None,
    git_runner: GitRunner | None,
    git_contains_cache: GitContainsCache,
) -> list[dict[str, Any]]:
    """Return conflicts that make a node requirement group unsafe to compare."""

    kinds = {
        requirement.version_kind
        for requirement in node_requirements
        if requirement.version_kind not in {"missing", "unknown"}
    }
    if len(kinds) > 1:
        return [
            {
                "reason": "mixed_version_kinds",
                "versions": _unique_sorted(
                    requirement.required_version for requirement in node_requirements
                ),
            }
        ]
    if (
        kinds == {"git_sha"}
        and installed is not None
        and installed.source_kind == "git"
    ):
        if installed.dirty:
            return []
        divergent = _divergent_git_requirements(
            node_requirements,
            repo_path=Path(installed.source_path),
            git_runner=git_runner,
            git_contains_cache=git_contains_cache,
        )
        if divergent:
            return [{"reason": "divergent_git_requirements", "versions": divergent}]
    return []


def _required_version(requirements: Sequence[CubeDependencyRequirement]) -> str:
    """Return the strongest required version when it can be chosen locally."""

    semver_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "semver"
    ]
    if semver_versions:
        return max(semver_versions, key=_semver_key)
    git_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "git_sha"
    ]
    if git_versions:
        return git_versions[-1]
    unknown_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "unknown"
    ]
    return unknown_versions[0] if unknown_versions else ""


def _strongest_semver_requirement(
    requirements: Sequence[CubeDependencyRequirement],
) -> str:
    """Return the highest semver-looking requirement from runtime facts."""

    semver_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "semver"
    ]
    if semver_versions:
        return max(semver_versions, key=_semver_key)
    versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.required_version
    ]
    return versions[0] if versions else ""


def _semver_status(
    required_version: str,
    installed: InstalledDependency,
) -> DependencyStatus:
    """Return semver readiness for one installed dependency."""

    if installed.version_kind == "missing":
        return "installed_version_unknown"
    if installed.version_kind != "semver":
        return "not_comparable"
    if _semver_key(installed.installed_version) >= _semver_key(required_version):
        return "satisfied"
    return "installed_version_too_old"


def _git_status(
    required_version: str,
    installed: InstalledDependency,
    *,
    git_runner: GitRunner | None,
    git_contains_cache: GitContainsCache,
) -> DependencyStatus:
    """Return git commit readiness for one installed dependency."""

    if installed.source_kind != "git":
        return "installed_version_unknown"
    if installed.dirty:
        return "blocked"
    if installed.version_kind != "git_sha":
        return "installed_version_unknown"
    if required_version == installed.installed_version:
        return "satisfied"
    if _git_commit_contains(
        repo_path=Path(installed.source_path),
        ancestor=required_version,
        descendant=installed.installed_version,
        git_runner=git_runner,
        git_contains_cache=git_contains_cache,
    ):
        return "satisfied"
    return "installed_commit_not_descendant"


def _installed_dependency(
    path: Path,
    *,
    git_runner: GitRunner | None,
    detailed_git: bool,
    phase_timings: dict[str, float],
) -> InstalledDependency:
    """Inspect one installed custom-node folder."""

    git_dir = path / ".git"
    phase_started_at = perf_counter()
    tracking = _read_tracking_metadata(path / ".tracking")
    _add_phase_time(phase_timings, "read_tracking_metadata", phase_started_at)
    phase_started_at = perf_counter()
    git_exists = git_dir.exists()
    _add_phase_time(phase_timings, "probe_git_dir", phase_started_at)
    if git_exists and git_runner is not None:
        if not detailed_git:
            return _cheap_git_dependency(
                path,
                tracking,
                phase_timings=phase_timings,
            )
        phase_started_at = perf_counter()
        head = _read_git_head(git_dir) or _git_stdout(
            ["rev-parse", "HEAD"], cwd=path, git_runner=git_runner
        )
        _add_phase_time(phase_timings, "read_git_head", phase_started_at)
        phase_started_at = perf_counter()
        dirty = bool(
            _git_stdout(["status", "--porcelain"], cwd=path, git_runner=git_runner)
        )
        _add_phase_time(phase_timings, "read_git_status", phase_started_at)
        phase_started_at = perf_counter()
        repository_url = _read_git_remote_origin_url(git_dir) or _git_stdout(
            ["config", "--get", "remote.origin.url"], cwd=path, git_runner=git_runner
        )
        _add_phase_time(phase_timings, "read_git_remote", phase_started_at)
        return InstalledDependency(
            folder_name=path.name,
            source_path=str(path),
            installed_version=head,
            version_kind=classify_version(head),
            source_kind="git",
            repository_url=repository_url,
            dirty=dirty,
        )
    return InstalledDependency(
        folder_name=path.name,
        source_path=str(path),
        installed_version=_normalize_text(tracking.get("version")),
        version_kind=classify_version(_normalize_text(tracking.get("version"))),
        source_kind="tracking" if tracking else "directory",
        repository_url=_normalize_text(tracking.get("repository")),
        dirty=False,
    )


def _cheap_git_dependency(
    path: Path,
    tracking: Mapping[str, Any],
    *,
    phase_timings: dict[str, float],
) -> InstalledDependency:
    """Return git evidence for non-required nodes without subprocess probes."""

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
    """Read a git HEAD value directly for non-authoritative evidence."""

    resolved_git_dir = _resolve_git_dir(git_dir)
    if resolved_git_dir is None:
        return ""
    git_dir = resolved_git_dir
    head = _read_text_file(git_dir / "HEAD")
    if head.startswith("ref:"):
        ref_name = head.removeprefix("ref:").strip()
        return _read_text_file(git_dir / ref_name)
    return head


def _read_git_remote_origin_url(git_dir: Path) -> str:
    """Read the origin URL directly when the repository uses a plain config."""

    resolved_git_dir = _resolve_git_dir(git_dir)
    if resolved_git_dir is None:
        return ""
    config_path = resolved_git_dir / "config"
    try:
        import configparser

        parser = configparser.ConfigParser()
        parser.read(config_path, encoding="utf-8")
        return _normalize_text(parser.get('remote "origin"', "url", fallback=""))
    except (OSError, configparser.Error):
        return ""


def _resolve_git_dir(git_path: Path) -> Path | None:
    """Return the concrete git metadata directory for repos and worktrees."""

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
    """Read best-effort Comfy Manager tracking metadata without trusting it as git."""

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


def _divergent_git_requirements(
    requirements: Sequence[CubeDependencyRequirement],
    *,
    repo_path: Path,
    git_runner: GitRunner | None,
    git_contains_cache: GitContainsCache,
) -> list[str]:
    """Return git SHA requirements that cannot be ordered by ancestry."""

    versions = _unique_sorted(
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "git_sha"
    )
    if len(versions) < 2 or git_runner is None:
        return []
    for left in versions:
        related = False
        for right in versions:
            if left == right:
                continue
            if _git_commit_contains(
                repo_path=repo_path,
                ancestor=left,
                descendant=right,
                git_runner=git_runner,
                git_contains_cache=git_contains_cache,
            ) or _git_commit_contains(
                repo_path=repo_path,
                ancestor=right,
                descendant=left,
                git_runner=git_runner,
                git_contains_cache=git_contains_cache,
            ):
                related = True
                break
        if not related:
            return versions
    return []


def _git_commit_contains(
    *,
    repo_path: Path,
    ancestor: str,
    descendant: str,
    git_runner: GitRunner | None,
    git_contains_cache: GitContainsCache,
) -> bool:
    """Return whether `descendant` contains `ancestor` in a git checkout."""

    if ancestor == descendant:
        return True
    if git_runner is None:
        return False
    key = (str(repo_path.resolve()), ancestor, descendant)
    cached = git_contains_cache.get(key)
    if cached is not None:
        return cached
    try:
        result = git_runner(
            ["merge-base", "--is-ancestor", ancestor, descendant],
            cwd=repo_path,
        )
    except (OSError, RuntimeError, ValueError) as exc:
        _logger.debug(
            "SugarCubes: git ancestry check failed",
            extra={
                "repo_path": str(repo_path),
                "ancestor": ancestor,
                "descendant": descendant,
                "error": str(exc),
            },
        )
        return False
    contains = int(getattr(result, "returncode", 0) or 0) == 0
    git_contains_cache[key] = contains
    return contains


def _git_stdout(args: Sequence[str], *, cwd: Path, git_runner: GitRunner) -> str:
    """Run a git inspection command and return stripped stdout."""

    try:
        result = git_runner(list(args), cwd=cwd)
    except (OSError, RuntimeError, ValueError):
        return ""
    return _normalize_text(getattr(result, "stdout", ""))


def _iter_workflow_nodes(value: Any) -> tuple[Mapping[str, Any], ...]:
    """Return workflow node dictionaries from nested cube payload shapes."""

    nodes: list[Mapping[str, Any]] = []
    if isinstance(value, Mapping):
        properties = value.get("properties")
        if isinstance(properties, Mapping) and "cnr_id" in properties:
            nodes.append(value)
        for child in value.values():
            nodes.extend(_iter_workflow_nodes(child))
    elif isinstance(value, list):
        for child in value:
            nodes.extend(_iter_workflow_nodes(child))
    return tuple(nodes)


def _iter_custom_node_slugs(payload: Mapping[str, Any]) -> tuple[str, ...]:
    """Return fallback custom-node module slugs from current and legacy payloads."""

    slugs: set[str] = set()
    for module_name in _iter_python_modules(payload):
        if not module_name.startswith("custom_nodes."):
            continue
        slug = module_name.split(".", 1)[1].strip()
        if slug and slug not in _EXCLUDED_CUSTOM_NODE_SLUGS:
            slugs.add(slug)
    return tuple(sorted(slugs))


def _iter_python_modules(payload: Mapping[str, Any]) -> tuple[str, ...]:
    """Return python module references from supported cube definition containers."""

    modules: list[str] = []
    for container_name in ("definitions",):
        definitions = payload.get(container_name)
        if isinstance(definitions, Mapping):
            modules.extend(_definition_modules(definitions))
    implementation = payload.get("implementation")
    if isinstance(implementation, Mapping):
        definitions = implementation.get("definitions")
        if isinstance(definitions, Mapping):
            modules.extend(_definition_modules(definitions))
    return tuple(modules)


def _definition_modules(definitions: Mapping[str, Any]) -> list[str]:
    """Return module strings from a definitions mapping."""

    modules: list[str] = []
    for spec in definitions.values():
        if not isinstance(spec, Mapping):
            continue
        module_name = spec.get("python_module")
        if isinstance(module_name, str) and module_name.strip():
            modules.append(module_name.strip())
    return modules


def _is_external_custom_node_requirement(value: str) -> bool:
    """Return whether a node id is an installable custom-node requirement."""

    normalized = _requirement_key(value)
    if not normalized:
        return False
    return (
        normalized not in _BUILT_IN_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_MARKER_MODULES
    )


def _requirement_key(value: str) -> str:
    """Return a stable key for comparing registry ids and folder names."""

    return re.sub(r"[-_.]+", "-", value.strip().casefold())


def _semver_key(value: str) -> tuple[int, int, int, int, str]:
    """Return a conservative sortable key for semver-like strings."""

    main, _, suffix = value.partition("-")
    numeric = [int(part) for part in main.split(".") if part.isdigit()]
    padded = [*numeric, 0, 0, 0, 0][:4]
    return (padded[0], padded[1], padded[2], padded[3], suffix)


def _remediation_for_status(status: DependencyStatus) -> str:
    """Return a short user-facing remediation for one version status."""

    if status == "installed_version_too_old":
        return "Update the installed custom node to the required version or newer."
    if status == "installed_commit_not_descendant":
        return "Update the installed git checkout to a commit containing the required cube commit."
    if status == "installed_version_unknown":
        return "Installed custom-node version could not be proven safely."
    if status == "blocked":
        return "Installed custom-node checkout is dirty or otherwise unsafe to mutate."
    if status == "not_comparable":
        return "Installed and required versions are not safely comparable."
    return ""


def _unique_sorted(values: Sequence[str] | Any) -> list[str]:
    """Return non-empty unique strings sorted case-insensitively."""

    return sorted(
        {value.strip() for value in values if isinstance(value, str) and value.strip()},
        key=str.casefold,
    )


def _normalize_text(value: object) -> str:
    """Return a stripped string or an empty string."""

    return value.strip() if isinstance(value, str) else ""
