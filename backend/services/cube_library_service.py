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
"""Source-aware cube library services and metadata summarization."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import re
import shutil
from copy import deepcopy
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from time import perf_counter
from typing import Any, Callable, Mapping, Optional, Sequence

try:
    from ...cube_model import (
        CubeIdentityError,
        RESERVED_SOURCE_NAMES,
        apply_cube_identity_projection,
        derive_route_from_cube_id,
        derive_target_model_from_cube_id,
        derive_source_author_label,
        looks_like_current_cube_payload,
        normalize_cube_route,
        normalize_supported_models as normalize_supported_model_labels,
        normalize_target_model,
        validate_cube_route_identity,
        parse_canonical_cube_id,
    )
    from ...importer import CubeImportError
    from ...instrumentation import log_diagnostic, log_event
    from ..responses import BackendError
    from .cube_icon_service import (
        CubeIconError,
        attach_icon_url,
        normalize_existing_icon_metadata,
        normalize_icon_metadata,
        resolve_icon_asset_path,
    )
    from .cube_catalog_state_service import CubeCatalogStateService
    from .dependency_versions import (
        CubeDependencyRequirement,
        classify_version,
        dependency_version_readiness,
        extract_versioned_requirements,
    )
    from .cube_git_context import CubeGitContext, resolve_cube_git_context
    from .cube_version_artifact_cache import (
        CubeVersionArtifactCache,
        CubeVersionArtifactCacheKey,
        CubeVersionSelectionCacheKey,
    )
    from .ownership_policy_service import OwnershipPolicyService
    from .tracked_repo_service import TrackedRepo, TrackedRepoService
except ImportError:
    from cube_model import (
        CubeIdentityError,
        RESERVED_SOURCE_NAMES,
        apply_cube_identity_projection,
        derive_route_from_cube_id,
        derive_target_model_from_cube_id,
        derive_source_author_label,
        normalize_cube_route,
        normalize_supported_models as normalize_supported_model_labels,
        normalize_target_model,
        validate_cube_route_identity,
        parse_canonical_cube_id,
    )
    from cube_model import looks_like_current_cube_payload
    from importer import CubeImportError
    from instrumentation import log_diagnostic, log_event
    from backend.responses import BackendError
    from backend.services.cube_icon_service import (
        CubeIconError,
        attach_icon_url,
        normalize_existing_icon_metadata,
        normalize_icon_metadata,
        resolve_icon_asset_path,
    )
    from backend.services.cube_catalog_state_service import CubeCatalogStateService
    from backend.services.dependency_versions import (
        CubeDependencyRequirement,
        classify_version,
        dependency_version_readiness,
        extract_versioned_requirements,
    )
    from backend.services.cube_git_context import (
        CubeGitContext,
        resolve_cube_git_context,
    )
    from backend.services.cube_version_artifact_cache import (
        CubeVersionArtifactCache,
        CubeVersionArtifactCacheKey,
        CubeVersionSelectionCacheKey,
    )
    from backend.services.ownership_policy_service import OwnershipPolicyService
    from backend.services.tracked_repo_service import TrackedRepo, TrackedRepoService

_logger = logging.getLogger(__name__)
CUBE_LIBRARY_TRACE_MARKER = "SugarCubes cube library diagnostic"
_LIBRARY_READINESS_CACHE_TTL_SECONDS = 30.0
_DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION = 1
_DEPENDENCY_REQUIREMENT_CACHE_FILENAME = "dependency-requirements.json"


class DuplicateCubeIdConflict(RuntimeError):
    """Retain the historical duplicate-id error type during the migration."""


_LOCAL_WORKSPACE_NAME = "local"
_TITLE_SMALL_WORDS = {
    "a",
    "an",
    "and",
    "as",
    "at",
    "but",
    "by",
    "for",
    "from",
    "in",
    "into",
    "nor",
    "of",
    "on",
    "or",
    "per",
    "the",
    "to",
    "vs",
    "via",
    "with",
}
_TITLE_TECH_TERMS = {
    "ai": "AI",
    "clip": "CLIP",
    "ipadapter": "IPAdapter",
    "lora": "LoRA",
    "sd": "SD",
    "sdxl": "SDXL",
    "ui": "UI",
    "vae": "VAE",
    "xl": "XL",
}
_ASCII_LETTER_RE = re.compile(r"[A-Za-z]")
_ASCII_WORD_RE = re.compile(r"[A-Za-z]+")
_VERSION_TOKEN_RE = re.compile(r"v[0-9]+(?:[A-Za-z0-9.-]*)?$", re.IGNORECASE)
_EXCLUDED_CUSTOM_NODE_SLUGS = frozenset({"websocket_image_save"})
_BUILT_IN_CUSTOM_NODE_IDS = frozenset({"comfy-core"})
_SUGARCUBES_CUSTOM_NODE_IDS = frozenset({"sugarcubes"})
_SUGARCUBES_MARKER_MODULES = frozenset({"nodes", "payloads"})
_DEFAULT_BASE_REPO_REF = "Artificial-Sweetener/Base-Cubes"
_CURRENT_REVISION_REF = "WORKTREE"


def format_timestamp(timestamp: float) -> str:
    """Return an ISO 8601 timestamp for UI metadata."""

    return datetime.fromtimestamp(timestamp, tz=timezone.utc).isoformat(
        timespec="seconds"
    )


def safe_relative_path(path: Path, base: Path) -> Optional[str]:
    """Return the path relative to `base` using forward slashes."""

    try:
        relative = path.relative_to(base)
    except ValueError:
        return None
    return str(relative).replace(os.sep, "/")


def _path_mtime_ns(path: Path) -> int:
    """Return a file timestamp suitable for cheap cache invalidation."""

    try:
        return path.stat().st_mtime_ns
    except OSError:
        return 0


def _git_status_path(line: str) -> str:
    """Return the normalized path component from one porcelain status line."""

    if len(line) < 4:
        return ""
    path = line[3:].strip()
    if " -> " in path:
        path = path.rsplit(" -> ", 1)[-1]
    return path.strip('"').replace("\\", "/")


def format_display_path(path: Path, extension_root: Path) -> str:
    """Return a human-friendly display path."""

    for candidate in (Path.cwd(), extension_root):
        try:
            relative = path.relative_to(candidate)
            return str(relative).replace(os.sep, "/")
        except ValueError:
            continue
    return str(path.resolve()).replace(os.sep, "/")


def normalize_metadata_string(value: Any) -> str:
    """Normalize metadata text fields to non-empty strings."""

    if isinstance(value, str):
        trimmed = value.strip()
        if trimmed:
            return trimmed
    return ""


def normalize_tag_value(value: str) -> str:
    """Normalize a tag into a stable lowercase slug."""

    if not isinstance(value, str):
        return ""
    tokens = re.findall(r"[0-9a-zA-Z]+", value.lower())
    if not tokens:
        return ""
    if all(len(token) == 1 for token in tokens):
        return "".join(tokens)
    return "-".join(tokens)


def normalize_tags(value: Any) -> list[str]:
    """Coerce metadata tags into a list of normalized strings."""

    if isinstance(value, list):
        tags: list[str] = []
        for item in value:
            if isinstance(item, str):
                cleaned = normalize_tag_value(item)
                if cleaned:
                    tags.append(cleaned)
        return tags
    if isinstance(value, str):
        return [
            part
            for part in (normalize_tag_value(segment) for segment in value.split(","))
            if part
        ]
    return []


def normalize_supported_models(value: Any, *, target_model: str = "") -> list[str]:
    """Coerce supported model metadata through the shared target-model policy."""

    return normalize_supported_model_labels(value, target_model=target_model)


def validate_target_model_matches_cube_id(cube_id: str, target_model: str) -> None:
    """Reject target metadata that disagrees with the cube's repo path."""

    normalized_cube_id = normalize_metadata_string(cube_id)
    normalized_target = normalize_target_model(target_model)
    if not normalized_target or not normalized_cube_id:
        return
    try:
        path_target = derive_target_model_from_cube_id(normalized_cube_id)
    except CubeIdentityError as exc:
        raise BackendError(str(exc), status=400) from exc
    if path_target != normalized_target:
        raise BackendError(
            "metadata.target_model must match the cube id path",
            status=400,
            details={
                "cube_id": normalized_cube_id,
                "target_model": normalized_target,
                "path_target_model": path_target,
            },
        )


def normalize_lineage_payload(value: Any) -> Optional[dict[str, str]]:
    """Normalize lineage metadata into a compact mapping."""

    if not isinstance(value, Mapping):
        return None
    result: dict[str, str] = {}
    for key in (
        "id",
        "name",
        "version",
        "revision_ref",
        "author",
        "author_url",
        "forked_at",
    ):
        normalized = normalize_metadata_string(value.get(key))
        if normalized:
            result[key] = normalized
    return result or None


def derive_source_author_from_identity(
    cube_id: str,
    *,
    source_kind: str,
    owner: str,
    repo: str,
    namespace: str,
) -> str:
    """Return the source-derived author label for browser metadata."""

    if cube_id:
        try:
            return derive_source_author_label(cube_id)
        except CubeIdentityError:
            pass
    if source_kind == "github" and owner and repo:
        return f"{owner}/{repo}"
    if source_kind == "local" and namespace:
        return "local"
    return ""


def normalize_default_alias(value: Any) -> str:
    """Normalize a route-based user-supplied cube default alias."""

    try:
        route = normalize_cube_route(value)
    except CubeIdentityError as exc:
        raise BackendError(str(exc), status=400) from exc
    if not route:
        return ""
    segments = route.split("/")
    if len(segments) > 1:
        try:
            segments[0] = normalize_target_model(segments[0]) or segments[0]
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
    return "/".join(segments)


def _log_cube_library_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured cube library diagnostic line in standard Comfy logs."""

    log_diagnostic(_logger, CUBE_LIBRARY_TRACE_MARKER, event, fields)


def _runtime_version() -> str:
    """Return the SugarCubes runtime version exposed to backend adapters."""

    try:
        from .. import __version__
    except ImportError:
        from backend import __version__
    return normalize_metadata_string(__version__)


def normalize_default_alias_title(value: Any) -> str:
    """Normalize an authored cube display title without changing path semantics."""

    if not isinstance(value, str):
        return ""
    cleaned = value.strip()
    if not cleaned:
        return ""
    if "_" in cleaned or not _ASCII_LETTER_RE.search(cleaned):
        return cleaned
    parts = re.split(r"\s+", cleaned)
    last_index = len(parts) - 1
    return " ".join(
        _normalize_title_token(part, index=index, is_last=index == last_index)
        for index, part in enumerate(parts)
    )


def _normalize_title_token(token: str, *, index: int, is_last: bool) -> str:
    """Return one conservative title-cased token for authored cube names."""

    word_match = _ASCII_WORD_RE.search(token)
    if not word_match:
        return token
    word = word_match.group(0)
    word_lower = word.lower()
    if word_lower in _TITLE_TECH_TERMS:
        replacement = _TITLE_TECH_TERMS[word_lower]
    elif index > 0 and not is_last and word_lower in _TITLE_SMALL_WORDS:
        replacement = word_lower
    elif _VERSION_TOKEN_RE.fullmatch(token):
        replacement = token.lower()
        return replacement
    elif _has_mixed_casing_beyond_first_character(word):
        return token
    else:
        replacement = word[:1].upper() + word[1:].lower()
    return f"{token[: word_match.start()]}{replacement}{token[word_match.end() :]}"


def _has_mixed_casing_beyond_first_character(word: str) -> bool:
    """Return whether a token appears intentionally cased already."""

    return any(character.isupper() for character in word[1:]) and any(
        character.islower() for character in word
    )


def normalize_metadata_update(
    payload: Mapping[str, Any], *, cube_id: str = ""
) -> tuple[dict[str, Any], set[str]]:
    """Normalize metadata edit payloads into updates and removals."""

    normalized: dict[str, Any] = {}
    remove_keys: set[str] = set()
    for key in ("author_url",):
        if key not in payload:
            continue
        value = normalize_metadata_string(payload.get(key))
        if value:
            normalized[key] = value
        else:
            remove_keys.add(key)
    if "default_alias" in payload:
        default_alias = normalize_default_alias(payload.get("default_alias"))
        if default_alias:
            if cube_id:
                try:
                    validate_cube_route_identity(cube_id, default_alias)
                except CubeIdentityError as exc:
                    raise BackendError(str(exc), status=400) from exc
            normalized["default_alias"] = default_alias
        else:
            remove_keys.add("default_alias")
    if "tags" in payload:
        tags = normalize_tags(payload.get("tags"))
        if tags:
            normalized["tags"] = tags
        else:
            remove_keys.add("tags")
    if "target_model" in payload:
        try:
            target_model = normalize_target_model(payload.get("target_model"))
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        if target_model:
            validate_target_model_matches_cube_id(cube_id, target_model)
            normalized["target_model"] = target_model
        else:
            if cube_id:
                try:
                    path_target = derive_target_model_from_cube_id(cube_id)
                except CubeIdentityError as exc:
                    raise BackendError(str(exc), status=400) from exc
                if path_target:
                    raise BackendError(
                        "metadata.target_model is required for target-folder cubes",
                        status=400,
                        details={"cube_id": cube_id, "path_target_model": path_target},
                    )
            remove_keys.add("target_model")
    if "supported_models" in payload:
        target_model = normalize_metadata_string(normalized.get("target_model"))
        if not target_model and cube_id:
            try:
                target_model = derive_target_model_from_cube_id(cube_id)
            except CubeIdentityError as exc:
                raise BackendError(str(exc), status=400) from exc
        supported_models = normalize_supported_models(
            payload.get("supported_models"),
            target_model=target_model,
        )
        if supported_models:
            normalized["supported_models"] = supported_models
        else:
            remove_keys.add("supported_models")
    if "lineage" in payload:
        lineage = normalize_lineage_payload(payload.get("lineage"))
        if lineage:
            normalized["lineage"] = lineage
        else:
            remove_keys.add("lineage")
    if "icon" in payload:
        icon_value = payload.get("icon")
        if icon_value is None:
            remove_keys.add("icon")
        else:
            try:
                icon = normalize_icon_metadata(icon_value)
            except CubeIconError as exc:
                raise BackendError(str(exc), status=400) from exc
            if icon:
                normalized["icon"] = icon
            else:
                remove_keys.add("icon")
    return normalized, remove_keys


def apply_cube_version(payload: Mapping[str, Any], version: str) -> None:
    """Apply a suggested version to a mutable cube payload."""

    if version and isinstance(payload, dict):
        payload["version"] = version


def read_cube_payload(path: Path) -> tuple[Optional[Mapping[str, Any]], Optional[str]]:
    """Read a lightweight cube payload from disk."""

    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError:
        return None, "Cube file is not valid JSON"
    except (OSError, UnicodeDecodeError) as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to read cube %s", path, exc_info=exc)
        return None, str(exc)
    if not isinstance(payload, Mapping):
        return None, "Cube root must be a JSON object"
    return dict(payload), None


def read_cube_payload_with_hash(
    path: Path,
) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]:
    """Read a cube payload and content hash with one filesystem read."""

    try:
        content = path.read_bytes()
    except OSError as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to read cube %s", path, exc_info=exc)
        return None, str(exc), ""
    try:
        payload = json.loads(content.decode("utf-8"))
    except UnicodeDecodeError as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to decode cube %s", path, exc_info=exc)
        return None, str(exc), ""
    except json.JSONDecodeError:
        return (
            None,
            "Cube file is not valid JSON",
            compute_cube_content_hash_bytes(content),
        )
    if not isinstance(payload, Mapping):
        return (
            None,
            "Cube root must be a JSON object",
            compute_cube_content_hash_bytes(content),
        )
    return dict(payload), None, compute_cube_content_hash_bytes(content)


def compute_cube_content_hash(path: Path) -> str:
    """Return a stable content hash for one source-owned cube artifact."""

    return compute_cube_content_hash_bytes(path.read_bytes())


def compute_cube_content_hash_bytes(content: bytes) -> str:
    """Return a stable content hash for one cube artifact payload."""

    digest = hashlib.sha256()
    digest.update(b"sugarcube\0")
    digest.update(content)
    return f"sha256:{digest.hexdigest()}"


def iter_custom_node_slugs(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return custom-node slugs referenced by supported cube definition shapes."""

    slugs: set[str] = set()
    for module_name in _iter_python_modules(payload):
        if not module_name.startswith("custom_nodes."):
            continue
        slug = module_name.split(".", 1)[1].strip()
        if slug and slug not in _EXCLUDED_CUSTOM_NODE_SLUGS:
            slugs.add(slug)
    return tuple(sorted(slugs))


def iter_custom_node_requirement_ids(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return install-oriented custom-node ids required by a cube payload."""

    cnr_ids = {
        requirement
        for requirement in _iter_cnr_ids(payload)
        if _is_external_custom_node_requirement(requirement)
    }
    module_slugs = {
        slug
        for slug in iter_custom_node_slugs(payload)
        if _is_external_custom_node_requirement(slug)
    }
    normalized_cnr_ids = {
        _normalize_requirement_key(requirement) for requirement in cnr_ids
    }
    requirements = set(cnr_ids)
    for slug in module_slugs:
        if _normalize_requirement_key(slug) not in normalized_cnr_ids:
            requirements.add(slug)
    return tuple(sorted(requirements, key=str.casefold))


def _iter_cnr_ids(value: Any) -> Sequence[str]:
    """Return all Comfy Registry ids embedded in current cube payload shapes."""

    found: list[str] = []
    if isinstance(value, Mapping):
        cnr_id = value.get("cnr_id")
        if isinstance(cnr_id, str) and cnr_id.strip():
            found.append(cnr_id.strip())
        for child in value.values():
            found.extend(_iter_cnr_ids(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(_iter_cnr_ids(child))
    return tuple(found)


def _is_external_custom_node_requirement(value: str) -> bool:
    """Return whether a requirement should be handled as an installable node."""

    normalized = _normalize_requirement_key(value)
    if not normalized:
        return False
    return (
        normalized not in _BUILT_IN_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_MARKER_MODULES
    )


def _normalize_requirement_key(value: str) -> str:
    """Return a stable comparison key for custom-node ids and folder names."""

    return re.sub(r"[-_.]+", "-", value.strip().casefold())


def _iter_python_modules(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return python module references from current and legacy cube payloads."""

    modules: list[str] = []
    definitions = payload.get("definitions")
    if isinstance(definitions, Mapping):
        for spec in definitions.values():
            module_name = (
                spec.get("python_module") if isinstance(spec, Mapping) else None
            )
            if isinstance(module_name, str):
                modules.append(module_name)
        return tuple(modules)

    implementation = payload.get("implementation")
    implementation_definitions = (
        implementation.get("definitions")
        if isinstance(implementation, Mapping)
        else None
    )
    if isinstance(implementation_definitions, Mapping):
        for spec in implementation_definitions.values():
            module_name = (
                spec.get("python_module") if isinstance(spec, Mapping) else None
            )
            if isinstance(module_name, str):
                modules.append(module_name)
        return tuple(modules)

    for value in payload.values():
        if not isinstance(value, Mapping):
            continue
        module_name = value.get("python_module")
        if isinstance(module_name, str):
            modules.append(module_name)
    return tuple(modules)


def _create_registry_or_none(
    base_dir: Path,
    registry_factory: Optional[Callable[[Path], Any]],
) -> Optional[Any]:
    """Create the optional registry accelerator or fall back to direct scanning.

    The on-disk cube library remains authoritative. Registry initialization only
    speeds up lookups, so startup failures are logged and downgraded to a scan.
    """

    if registry_factory is None:
        return None
    try:
        return registry_factory(base_dir)
    except Exception as exc:  # pragma: no cover - registry integrations vary by host
        _logger.warning(
            "SugarCubes: failed to initialize cube registry for %s; falling back to directory scan",
            base_dir,
            exc_info=exc,
        )
        return None


def _cleanup_failed_import(path: Path) -> None:
    """Remove a failed imported cube when cleanup is still possible."""

    if not path.exists():
        return
    try:
        path.unlink()
    except OSError:
        _logger.warning(
            "SugarCubes: unable to clean up failed import %s",
            path,
            exc_info=True,
        )


def derive_cube_display_name(
    payload: Optional[Mapping[str, Any]], fallback: str
) -> str:
    """Resolve the browser display name for a cube."""

    if not payload:
        return fallback
    metadata_raw = payload.get("metadata")
    if isinstance(metadata_raw, Mapping):
        metadata_name = normalize_metadata_string(metadata_raw.get("default_alias"))
        if metadata_name:
            return metadata_name
    layout_raw = _read_layout_payload(payload)
    if isinstance(layout_raw, Mapping):
        groups = layout_raw.get("groups")
        if isinstance(groups, Sequence):
            names: list[str] = []
            for group in groups:
                if not isinstance(group, Mapping):
                    continue
                sugarcubes = group.get("sugarcubes")
                if isinstance(sugarcubes, Mapping):
                    name = normalize_metadata_string(sugarcubes.get("default_alias"))
                    if name:
                        names.append(name)
            unique_names = list(dict.fromkeys(names))
            if len(unique_names) == 1:
                return unique_names[0]
    return fallback


def build_cube_identity_fields(
    *,
    cube_id: str,
    default_alias: str,
    metadata: Optional[Mapping[str, Any]] = None,
) -> dict[str, str]:
    """Return normalized route-based identity fields for API payloads."""

    normalized_cube_id = normalize_metadata_string(cube_id)
    normalized_default_alias = normalize_metadata_string(default_alias)
    if normalized_cube_id:
        try:
            route_alias = derive_route_from_cube_id(normalized_cube_id)
        except CubeIdentityError:
            route_alias = normalized_default_alias
        else:
            normalized_default_alias = route_alias

    metadata_target_model = ""
    if isinstance(metadata, Mapping):
        try:
            metadata_target_model = normalize_target_model(metadata.get("target_model"))
        except CubeIdentityError:
            metadata_target_model = ""
    try:
        path_target_model = derive_target_model_from_cube_id(normalized_cube_id)
    except CubeIdentityError:
        path_target_model = ""
    target_model = path_target_model or metadata_target_model

    return {
        "default_alias": normalized_default_alias,
        "display_name": normalized_default_alias,
        "target_model": target_model,
    }


def summarize_cube_file(
    path: Path,
    base_dir: Path,
    extension_root: Path,
    *,
    source_kind: str,
    owner: str = "",
    repo: str = "",
    namespace: str = "",
    include_internal_payload: bool = False,
) -> dict[str, Any]:
    """Return lightweight metadata about one source-owned cube file."""

    if include_internal_payload:
        payload, error, content_hash = read_cube_payload_with_hash(path)
    else:
        payload, error = read_cube_payload(path)
        content_hash = ""
    stat_info = path.stat()
    description = ""
    metadata: dict[str, Any] = {}
    tags: list[str] = []
    supported_models: list[str] = []
    target_model = ""
    cube_id = ""
    version = ""
    author = ""
    author_url = ""
    icon: Optional[dict[str, str]] = None
    lineage: Optional[dict[str, str]] = None
    layout_present = False
    layout_nodes = 0
    layout_markers = 0
    layout_groups = 0

    if payload:
        raw_description = payload.get("description")
        if isinstance(raw_description, str):
            description = raw_description.strip()
        cube_id = normalize_metadata_string(payload.get("cube_id"))
        version = normalize_metadata_string(payload.get("version"))
        metadata_raw = payload.get("metadata")
        if isinstance(metadata_raw, Mapping):
            metadata = dict(metadata_raw)
            tags = normalize_tags(metadata.get("tags"))
            author_url = normalize_metadata_string(metadata.get("author_url"))
            lineage = normalize_lineage_payload(metadata.get("lineage"))
            icon = attach_icon_url(
                normalize_existing_icon_metadata(metadata.get("icon")),
                cube_id,
            )
        layout_raw = _read_layout_payload(payload)
        if isinstance(layout_raw, Mapping):
            nodes_map = layout_raw.get("nodes")
            if isinstance(nodes_map, Mapping):
                layout_nodes = len(nodes_map)
            markers_map = layout_raw.get("markers")
            if isinstance(markers_map, Mapping):
                layout_markers = len(markers_map)
            groups_list = layout_raw.get("groups")
            if isinstance(groups_list, Sequence):
                layout_groups = len(list(groups_list))
            layout_present = bool(layout_nodes or layout_markers or layout_groups)

    author = derive_source_author_from_identity(
        cube_id,
        source_kind=source_kind,
        owner=owner,
        repo=repo,
        namespace=namespace,
    )
    default_alias = derive_cube_display_name(payload, path.stem)
    display_fields = build_cube_identity_fields(
        cube_id=cube_id,
        default_alias=default_alias,
        metadata=metadata,
    )
    target_model = display_fields["target_model"]
    supported_models = normalize_supported_models(
        metadata.get("supported_models") if metadata else [],
        target_model=target_model,
    )

    entry = {
        "name": path.stem,
        "default_alias": display_fields["default_alias"],
        "display_name": display_fields["display_name"],
        "target_model": target_model,
        "filename": path.name,
        "path": format_display_path(path, extension_root),
        "relative_path": safe_relative_path(path, base_dir),
        "size_bytes": stat_info.st_size,
        "mtime": format_timestamp(stat_info.st_mtime),
        "mtime_ns": stat_info.st_mtime_ns,
        "description": description,
        "tags": tags,
        "supported_models": supported_models,
        "cube_id": cube_id,
        "version": version,
        "author": author,
        "author_url": author_url,
        "owner": owner,
        "repo": repo,
        "namespace": namespace,
        "layout": {
            "present": layout_present,
            "nodes": layout_nodes,
            "markers": layout_markers,
            "groups": layout_groups,
        },
        "source": {
            "type": source_kind,
            "owner": owner,
            "repo": repo,
            "namespace": namespace,
            "repo_ref": f"{owner}/{repo}" if owner and repo else "",
            "repo_relative_path": safe_relative_path(path, base_dir),
        },
    }
    if metadata:
        entry["metadata"] = metadata
    if icon:
        entry["icon"] = icon
    if lineage:
        entry["lineage"] = lineage
    if error:
        entry["error"] = error
    if include_internal_payload:
        entry["_absolute_path"] = str(path.resolve())
        entry["_content_hash"] = content_hash
        entry["_payload"] = dict(payload) if payload else None
    return entry


def _read_layout_payload(
    payload: Optional[Mapping[str, Any]],
) -> Optional[Mapping[str, Any]]:
    """Return the layout mapping regardless of whether the payload is current or legacy."""

    if not isinstance(payload, Mapping):
        return None
    if looks_like_current_cube_payload(payload):
        implementation = payload.get("implementation")
        if isinstance(implementation, Mapping):
            layout = implementation.get("layout")
            if isinstance(layout, Mapping):
                return layout
        return None
    layout = payload.get("layout")
    return layout if isinstance(layout, Mapping) else None


def list_cube_files(base_dir: Path) -> list[Path]:
    """Return all managed cube files, excluding backup folders."""

    if not base_dir.exists():
        return []
    files: list[Path] = []
    for path in base_dir.rglob("*.cube"):
        if not path.is_file():
            continue
        try:
            relative = path.relative_to(base_dir)
        except ValueError:
            relative = None
        if relative is not None:
            parts = [part.lower() for part in relative.parts if part]
            if parts and parts[0] in {"old", "backup", "_old", "_history"}:
                continue
        files.append(path.resolve())
    files.sort()
    return files


def normalize_target_filename(value: Any, source: Path) -> str:
    """Derive a safe destination filename for imported cubes."""

    candidate = str(value).strip() if isinstance(value, str) else ""
    if not candidate:
        candidate = source.name
    name = Path(candidate).name
    if not name:
        name = source.name or "cube.cube"
    if not name.lower().endswith(".cube"):
        name = f"{name}.cube"
    sanitized_chars = [
        ch if ch.isalnum() or ch in {"-", "_", ".", " "} else "_" for ch in name
    ]
    sanitized = "".join(sanitized_chars).strip(" ._")
    if not sanitized:
        sanitized = source.stem or "cube"
    if not sanitized.lower().endswith(".cube"):
        sanitized = f"{sanitized}.cube"
    return sanitized


def dedupe_warnings(messages: Sequence[str]) -> list[str]:
    """Deduplicate warnings while preserving order."""

    seen: set[str] = set()
    ordered: list[str] = []
    for message in messages:
        if not message or message in seen:
            continue
        seen.add(message)
        ordered.append(message)
    return ordered


def _utc_now() -> str:
    """Return the current UTC timestamp for library API payloads."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")


class CubeLibraryService:
    """Own source-aware cube listing, preview, import, delete, and path resolution."""

    def __init__(
        self,
        extension_root: Path,
        *,
        load_cube_artifact: Callable[[Path], Any],
        prepare_cube_import: Callable[..., Any],
        tracked_repo_service: TrackedRepoService,
        ownership_policy_service: OwnershipPolicyService,
        registry_factory: Optional[Callable[[Path], Any]] = None,
    ) -> None:
        """Initialize the cube library service."""

        self.extension_root = extension_root.resolve()
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.tracked_repo_service = tracked_repo_service
        self.ownership_policy_service = ownership_policy_service
        self.registry_factory = registry_factory
        self.version_artifact_cache = CubeVersionArtifactCache(
            self.extension_root / ".sugarcubes" / "cache" / "cube-version-artifacts"
        )
        self._version_warm_executor = ThreadPoolExecutor(
            max_workers=1,
            thread_name_prefix="sugarcubes-version-warm",
        )
        self._library_change_listeners: list[Callable[[dict[str, Any]], None]] = []
        self._catalog_state = CubeCatalogStateService(
            list_summaries=lambda include_disabled: self._list_catalog_cube_summaries(
                include_disabled=include_disabled
            ),
            build_entry=self._catalog_entry_for_summary,
            revision_pack_facts=lambda include_disabled: self._revision_pack_facts(
                include_disabled=include_disabled
            ),
            pack_counts=self._pack_counts,
            generated_at=_utc_now,
        )
        self._library_readiness_cache: (
            tuple[float, Path, str, dict[str, Any]] | None
        ) = None
        self._repo_dirty_paths_cache: dict[Path, frozenset[str]] = {}
        try:
            self.version_artifact_cache.prune()
        except (OSError, RuntimeError, TypeError, ValueError):
            _logger.warning(
                "SugarCubes: version artifact cache prune failed during startup",
                exc_info=True,
            )

    def subscribe_library_changed(
        self,
        listener: Callable[[dict[str, Any]], None],
    ) -> Callable[[], None]:
        """Register a generic library-change listener and return an unsubscribe."""

        self._library_change_listeners.append(listener)

        def unsubscribe() -> None:
            """Remove the registered library-change listener."""

            try:
                self._library_change_listeners.remove(listener)
            except ValueError:
                return

        return unsubscribe

    def notify_library_changed(
        self,
        *,
        affected_cube_ids: Sequence[str],
        saved_versions: Mapping[str, str],
        reason: str,
    ) -> None:
        """Publish a generic library-change event to in-process consumers."""

        self.invalidate_catalog_state(
            reason=reason,
            affected_cube_ids=affected_cube_ids,
        )
        event = {
            "schemaVersion": 1,
            "affectedCubeIds": list(affected_cube_ids),
            "savedVersions": dict(saved_versions),
            "generatedAt": _utc_now(),
            "reason": reason,
            "catalogRevision": self.catalog_revision(),
        }
        for listener in tuple(self._library_change_listeners):
            try:
                listener(dict(event))
            except (RuntimeError, TypeError, ValueError):
                _logger.exception("SugarCubes library change listener failed")

    def invalidate_catalog_state(
        self,
        *,
        reason: str,
        affected_cube_ids: Sequence[str] = (),
    ) -> None:
        """Invalidate cached catalog state after a library-visible mutation."""

        self._catalog_state.invalidate(
            reason,
            affected_cube_ids=affected_cube_ids,
        )
        self._library_readiness_cache = None
        self._repo_dirty_paths_cache.clear()

    def repo_workspace_root(self) -> Path:
        """Return the managed tracked-repo workspace root."""

        return self.tracked_repo_service.workspace_root()

    def local_workspace_root(self) -> Path:
        """Return the managed local source workspace root."""

        return self.tracked_repo_service.ensure_local_repo()

    def list_cubes(self) -> dict[str, Any]:
        """Return the source-aware cube library response payload."""

        try:
            cubes: list[dict[str, Any]] = []
            repo_entries = self.tracked_repo_service.list_repos()["repos"]
            for repo_entry in repo_entries:
                if not repo_entry.get("enabled"):
                    continue
                tracked = TrackedRepo(
                    owner=repo_entry["owner"],
                    repo=repo_entry["repo"],
                    branch=repo_entry["branch"],
                    enabled=bool(repo_entry["enabled"]),
                    default_base_repo=bool(repo_entry["default_base_repo"]),
                    local_checkout_path=repo_entry["local_checkout_path"],
                    last_sync_at=repo_entry["last_sync_at"],
                    last_sync_status=repo_entry["last_sync_status"],
                    last_sync_error=repo_entry["last_sync_error"],
                )
                cubes.extend(self._list_repo_cubes(tracked))
            cubes.extend(self._list_local_cubes())
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: failed to list cubes")
            raise BackendError("Failed to list SugarCubes", status=500) from exc

        log_event(
            "frontend.phase5",
            "list_cubes",
            {
                "count": len(cubes),
                "directory": format_display_path(
                    self.repo_workspace_root(), self.extension_root
                ),
            },
        )
        return {
            "cubes": cubes,
            "directory": format_display_path(
                self.repo_workspace_root(), self.extension_root
            ),
            "exists": self.repo_workspace_root().exists()
            or self.local_workspace_root().exists(),
            "count": len(cubes),
        }

    def library_status(self) -> dict[str, Any]:
        """Return target-owned Cube Library availability for backend adapters."""

        try:
            catalog_revision = self.catalog_revision()
            available = True
            errors: list[dict[str, str]] = []
        except BackendError as exc:
            available = False
            catalog_revision = ""
            errors = [{"code": "catalog-unavailable", "message": exc.message}]
        except (OSError, RuntimeError, TypeError, ValueError):
            _logger.exception("SugarCubes: library status failed")
            available = False
            catalog_revision = ""
            errors = [
                {
                    "code": "catalog-unavailable",
                    "message": "SugarCubes catalog is unavailable.",
                }
            ]
        return {
            "schemaVersion": 1,
            "available": available,
            "source": "SugarCubes",
            "sugarCubesVersion": _runtime_version(),
            "catalogRevision": catalog_revision,
            "packManagementSupported": available,
            "localAuthoringSupported": available,
            "readinessSupported": available,
            "dependencyReadinessSupported": available,
            "dependencyRepairSupported": available,
            "versionedDependencyReadinessSupported": available,
            "syncDependencyOrchestrationSupported": available,
            "errors": errors,
        }

    def library_capabilities_status(self) -> dict[str, Any]:
        """Return Cube Library capability facts without building catalog state."""

        return {
            "schemaVersion": 1,
            "available": True,
            "source": "SugarCubes",
            "sugarCubesVersion": _runtime_version(),
            "catalogRevision": "",
            "packManagementSupported": True,
            "localAuthoringSupported": True,
            "readinessSupported": True,
            "dependencyReadinessSupported": True,
            "dependencyRepairSupported": True,
            "versionedDependencyReadinessSupported": True,
            "syncDependencyOrchestrationSupported": True,
            "errors": [],
        }

    def catalog_revision(self, *, include_disabled: bool = False) -> str:
        """Return a deterministic revision for catalog-relevant library state."""

        revision = self._catalog_state.current_revision(
            include_disabled=include_disabled
        )
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_revision",
            include_disabled=include_disabled,
            catalog_revision=revision,
        )
        return revision

    def list_library_catalog(self, *, include_disabled: bool = False) -> dict[str, Any]:
        """Return backend-facing catalog metadata for enabled library cubes."""

        _log_cube_library_diagnostic(
            "sugarcubes_list_catalog_start",
            include_disabled=include_disabled,
        )
        payload = self._catalog_state.current_catalog(include_disabled=include_disabled)
        _log_cube_library_diagnostic(
            "sugarcubes_list_catalog_return",
            include_disabled=include_disabled,
            cube_count=len(payload["cubes"]),
            catalog_revision=payload["catalogRevision"],
        )
        return payload

    def load_library_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the canonical cube document and source metadata for one cube id."""

        started_at = perf_counter()
        _log_cube_library_diagnostic(
            "sugarcubes_load_library_cube_start",
            requested_cube_id=cube_id,
            version_ref_supported=False,
            revision_ref_supported=False,
        )
        context = self._resolve_cube_ref_context(cube_id)
        response = self._load_current_library_artifact(context)
        _log_cube_library_diagnostic(
            "sugarcubes_load_library_cube_return",
            requested_cube_id=cube_id,
            loaded_cube_id=response["cubeId"],
            loaded_version=response["version"],
            content_hash=response["contentHash"],
            cube_path=format_display_path(context.cube_path, self.extension_root),
            duration_ms=round((perf_counter() - started_at) * 1000, 3),
        )
        return response

    def list_library_cube_refs(self, cube_id: str) -> dict[str, Any]:
        """Return exact artifact refs available for one cube id."""

        context = self._resolve_cube_ref_context(cube_id)
        refs = [self._current_cube_ref(context)]
        refs.extend(self._committed_cube_refs(context))
        return {
            "schemaVersion": 1,
            "cubeId": context.cube_id,
            "refs": refs,
            "count": len(refs),
        }

    def list_library_cube_versions(self, cube_id: str) -> dict[str, Any]:
        """Return unique versions available for one cube id, newest first."""

        context = self._resolve_cube_ref_context(cube_id)
        refs = [self._current_cube_ref(context)]
        refs.extend(self._committed_cube_refs(context))
        versions: list[str] = []
        for ref in refs:
            version = normalize_metadata_string(ref.get("version"))
            if version and version not in versions:
                versions.append(version)
        return {
            "schemaVersion": 1,
            "cubeId": context.cube_id,
            "versions": versions,
            "count": len(versions),
        }

    def load_library_cube_version(
        self,
        *,
        cube_id: str,
        version: str,
    ) -> dict[str, Any]:
        """Load the newest artifact for a cube id and version."""

        started_at = perf_counter()
        normalized_version = normalize_metadata_string(version)
        if not normalized_version:
            raise BackendError("Cube version is required", status=400)
        context = self._resolve_cube_ref_context(cube_id)
        current_artifact = self._load_current_library_artifact(context)
        if (
            normalize_metadata_string(current_artifact.get("version"))
            == normalized_version
        ):
            _log_cube_library_diagnostic(
                "sugarcubes_load_library_cube_version_return",
                requested_cube_id=cube_id,
                loaded_cube_id=current_artifact["cubeId"],
                loaded_version=current_artifact["version"],
                resolution="current",
                duration_ms=round((perf_counter() - started_at) * 1000, 3),
            )
            return current_artifact
        artifact = self._load_cached_or_historical_library_artifact(
            context,
            version=normalized_version,
        )
        self._assert_loaded_artifact_matches_ref(
            artifact,
            cube_id=context.cube_id,
            revision_ref="",
            content_hash="",
            version=normalized_version,
        )
        _log_cube_library_diagnostic(
            "sugarcubes_load_library_cube_version_return",
            requested_cube_id=cube_id,
            loaded_cube_id=artifact["cubeId"],
            loaded_version=artifact["version"],
            resolution="historical",
            duration_ms=round((perf_counter() - started_at) * 1000, 3),
        )
        return artifact

    def load_library_cube_ref(
        self,
        *,
        cube_id: str,
        revision_ref: str = "",
        content_hash: str = "",
        version: str = "",
    ) -> dict[str, Any]:
        """Load a Cube Library artifact by an exact or uniquely resolved selector."""

        context = self._resolve_cube_ref_context(cube_id)
        selected = self._select_cube_ref(
            context,
            revision_ref=normalize_metadata_string(revision_ref),
            content_hash=normalize_metadata_string(content_hash),
            version=normalize_metadata_string(version),
        )
        if selected["revisionRef"] == _CURRENT_REVISION_REF:
            artifact = self.load_library_cube(context.cube_id)
        else:
            artifact = self._load_revision_library_artifact(
                context,
                revision_ref=str(selected["revisionRef"]),
            )
        self._assert_loaded_artifact_matches_ref(
            artifact,
            cube_id=context.cube_id,
            revision_ref=str(selected["revisionRef"]),
            content_hash=normalize_metadata_string(content_hash),
            version=normalize_metadata_string(version),
        )
        return artifact

    def warm_library_cube_version(self, *, cube_id: str, version: str) -> None:
        """Schedule a best-effort historical cube version cache fill."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        normalized_version = normalize_metadata_string(version)
        if not normalized_cube_id or not normalized_version:
            raise BackendError("Cube id and version are required", status=400)
        self._version_warm_executor.submit(
            self._warm_library_cube_version,
            normalized_cube_id,
            normalized_version,
        )

    def _warm_library_cube_version(self, cube_id: str, version: str) -> None:
        """Fill the version cache without surfacing background failures."""

        try:
            self.load_library_cube_version(cube_id=cube_id, version=version)
        except (BackendError, OSError, RuntimeError, TypeError, ValueError):
            _logger.exception(
                "SugarCubes cube version prewarm failed",
                extra={"cube_id": cube_id, "version": version},
            )

    def list_library_packs(self) -> dict[str, Any]:
        """Return tracked Cube Pack records without exposing checkout paths."""

        packs = [
            self._pack_record(repo_entry)
            for repo_entry in self.tracked_repo_service.list_repos()["repos"]
        ]
        packs.sort(key=lambda pack: str(pack.get("repoRef", "")).casefold())
        return {
            "schemaVersion": 1,
            "packs": packs,
            "catalogRevision": self.catalog_revision(),
        }

    def preflight_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Return preflight information for a candidate Cube Pack."""

        payload = self.tracked_repo_service.preflight_repo(
            owner=owner,
            repo=repo,
            branch=branch,
        )
        return {"schemaVersion": 1, **payload}

    def add_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
        enabled: bool,
        auto_update: bool,
        sync_immediately: bool,
    ) -> dict[str, Any]:
        """Track a Cube Pack and optionally perform the first synchronous sync."""

        payload = self.tracked_repo_service.add_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            default_base_repo=False,
            auto_update=auto_update,
        )
        if sync_immediately:
            payload = {
                **payload,
                "repo": self.tracked_repo_service.sync_repo(
                    owner=owner,
                    repo=repo,
                )["repo"],
            }
        self.invalidate_catalog_state(reason="pack_added")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "preflight": payload.get("preflight", {}),
            "catalogRevision": self.catalog_revision(),
        }

    def update_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str | None,
        enabled: bool | None,
        auto_update: bool | None,
    ) -> dict[str, Any]:
        """Update a tracked Cube Pack and return refreshed library state."""

        payload = self.tracked_repo_service.update_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )
        self.invalidate_catalog_state(reason="pack_updated")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def remove_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove a tracked Cube Pack through SugarCubes policy enforcement."""

        payload = self.tracked_repo_service.remove_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_removed")
        return {
            "schemaVersion": 1,
            **payload,
            "catalogRevision": self.catalog_revision(),
        }

    def sync_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Synchronously sync one tracked Cube Pack."""

        payload = self.tracked_repo_service.sync_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_synced")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def sync_all_library_packs(self) -> dict[str, Any]:
        """Synchronously sync all enabled Cube Packs and return per-pack results."""

        payload = self.tracked_repo_service.sync_all_repos()
        self.invalidate_catalog_state(reason="all_packs_synced")
        return {
            "schemaVersion": 1,
            "packs": [self._pack_record(repo) for repo in payload["repos"]],
            "catalogRevision": self.catalog_revision(),
        }

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
            _log_cube_library_diagnostic(
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
        installed_keys = {_normalize_requirement_key(slug) for slug in installed}
        missing = tuple(
            slug
            for slug in required
            if _normalize_requirement_key(slug) not in installed_keys
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
            git_runner=self.tracked_repo_service.git_runner,
        )
        record_phase("dependency_version_readiness")
        total_duration_ms = round((perf_counter() - started_at) * 1000, 3)
        _log_cube_library_diagnostic(
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
                if _normalize_requirement_key(slug) in installed_keys
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
            entries = ()
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

    def summarize_cube(self, cube_path: Path) -> dict[str, Any]:
        """Summarize a single cube file for browser payloads."""

        source = self.resolve_source_descriptor_by_path(cube_path)
        summary = summarize_cube_file(
            cube_path,
            Path(source["base_dir"]),
            self.extension_root,
            source_kind=source["source_kind"],
            owner=source["owner"],
            repo=source["repo"],
            namespace=source["namespace"],
        )
        return self.ownership_policy_service.annotate_cube_payload(summary)

    def _resolve_cube_ref_context(self, cube_id: str) -> CubeGitContext:
        """Resolve a cube ref context and verify the current file exists."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        context = resolve_cube_git_context(self.tracked_repo_service, normalized)
        if not context.cube_path.exists() or not context.cube_path.is_file():
            raise BackendError(f"Cube '{normalized}' not found", status=404)
        return context

    def _load_current_library_artifact(
        self,
        context: CubeGitContext,
    ) -> dict[str, Any]:
        """Return the backend-facing artifact for the current cube file."""

        payload, error = read_cube_payload(context.cube_path)
        if error or not payload:
            raise BackendError(error or "Cube could not be read", status=409)
        summary = self.summarize_cube(context.cube_path)
        content_hash = compute_cube_content_hash(context.cube_path)
        return {
            "schemaVersion": 1,
            "cubeId": normalize_metadata_string(payload.get("cube_id"))
            or context.cube_id,
            "version": normalize_metadata_string(payload.get("version")),
            "displayName": normalize_metadata_string(summary.get("display_name")),
            "targetModel": normalize_metadata_string(summary.get("target_model")),
            "supportedModels": list(summary.get("supported_models") or []),
            "contentHash": content_hash,
            "source": self._source_metadata_for_summary(summary),
            "cube": dict(payload),
        }

    def _current_cube_ref(self, context: CubeGitContext) -> dict[str, Any]:
        """Build the exact ref for the current working-tree cube artifact."""

        payload, error = read_cube_payload(context.cube_path)
        if error or not payload:
            raise BackendError(error or "Cube could not be read", status=409)
        stat_info = context.cube_path.stat()
        return self._cube_ref_payload(
            context,
            revision_ref=_CURRENT_REVISION_REF,
            payload=payload,
            content_hash=compute_cube_content_hash(context.cube_path),
            current=True,
            committed=False,
            label="Current",
            timestamp=format_timestamp(stat_info.st_mtime),
        )

    def _committed_cube_refs(self, context: CubeGitContext) -> list[dict[str, Any]]:
        """Return exact refs for committed cube artifacts in git history."""

        if not (context.repo_root / ".git").exists():
            return []
        try:
            result = self.tracked_repo_service.git_runner(
                [
                    "log",
                    "--format=%H%x1f%cI%x1f%s",
                    "--",
                    context.repo_relative_path,
                ],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            message = str(exc)
            if (
                "does not have any commits yet" in message
                or "unknown revision" in message
            ):
                return []
            raise BackendError("Failed to list cube refs", status=500) from exc

        refs: list[dict[str, Any]] = []
        for line in (getattr(result, "stdout", "") or "").splitlines():
            parts = line.split("\x1f")
            if len(parts) != 3:
                continue
            revision_ref, timestamp, subject = parts
            payload_text = self._git_show_cube(context, revision_ref)
            payload = self._read_revision_payload(payload_text)
            refs.append(
                self._cube_ref_payload(
                    context,
                    revision_ref=revision_ref,
                    payload=payload,
                    content_hash=compute_cube_content_hash_bytes(
                        payload_text.encode("utf-8")
                    ),
                    current=False,
                    committed=True,
                    label=revision_ref[:7],
                    timestamp=timestamp,
                    subject=subject,
                )
            )
        return refs

    def _cube_ref_payload(
        self,
        context: CubeGitContext,
        *,
        revision_ref: str,
        payload: Mapping[str, Any],
        content_hash: str,
        current: bool,
        committed: bool,
        label: str,
        timestamp: str,
        subject: str = "",
    ) -> dict[str, Any]:
        """Build one backend-facing exact cube ref payload."""

        return {
            "cubeId": normalize_metadata_string(payload.get("cube_id"))
            or context.cube_id,
            "version": normalize_metadata_string(payload.get("version")),
            "contentHash": content_hash,
            "revisionRef": revision_ref,
            "label": label,
            "current": current,
            "committed": committed,
            "timestamp": timestamp,
            "source": self._source_metadata_for_context(context),
            "subject": subject,
        }

    def _select_cube_ref(
        self,
        context: CubeGitContext,
        *,
        revision_ref: str,
        content_hash: str,
        version: str,
    ) -> dict[str, Any]:
        """Select one exact cube ref or fail closed with typed details."""

        refs = list(self.list_library_cube_refs(context.cube_id)["refs"])
        matches = refs
        exact_selector_present = bool(revision_ref or content_hash)
        if revision_ref:
            matches = [ref for ref in matches if ref.get("revisionRef") == revision_ref]
            if not matches:
                raise BackendError(
                    "Cube revision ref was not found.",
                    status=404,
                    details={"cube_id": context.cube_id, "revision_ref": revision_ref},
                )
        if content_hash:
            matches = [ref for ref in matches if ref.get("contentHash") == content_hash]
            if not matches:
                if revision_ref:
                    raise BackendError(
                        "Cube revision ref and content hash do not identify the same artifact.",
                        status=409,
                        details={
                            "cube_id": context.cube_id,
                            "revision_ref": revision_ref,
                            "content_hash": content_hash,
                        },
                    )
                raise BackendError(
                    "Cube content hash was not found.",
                    status=404,
                    details={"cube_id": context.cube_id, "content_hash": content_hash},
                )
        if version:
            matches = [ref for ref in matches if ref.get("version") == version]
            if not matches:
                if exact_selector_present:
                    raise BackendError(
                        "Cube exact selector and version guard do not identify the same artifact.",
                        status=409,
                        details={
                            "cube_id": context.cube_id,
                            "revision_ref": revision_ref,
                            "content_hash": content_hash,
                            "version": version,
                        },
                    )
                raise BackendError(
                    "Cube version was not found.",
                    status=404,
                    details={"cube_id": context.cube_id, "version": version},
                )
            if not exact_selector_present:
                return matches[0]
        if not revision_ref and not content_hash and not version:
            return refs[0]
        if len(matches) == 1:
            return matches[0]
        raise BackendError(
            "Cube version selector is ambiguous.",
            status=409,
            details={
                "cube_id": context.cube_id,
                "version": version,
                "matches": matches,
            },
        )

    def _select_cube_version(
        self,
        context: CubeGitContext,
        *,
        version: str,
    ) -> dict[str, Any]:
        """Select the newest artifact matching one cube version."""

        refs = list(self.list_library_cube_refs(context.cube_id)["refs"])
        for ref in refs:
            if normalize_metadata_string(ref.get("version")) == version:
                return ref
        raise BackendError(
            "Cube version was not found.",
            status=404,
            details={"cube_id": context.cube_id, "version": version},
        )

    def _load_cached_or_historical_library_artifact(
        self,
        context: CubeGitContext,
        *,
        version: str,
    ) -> dict[str, Any]:
        """Load a historical version through the durable artifact cache."""

        selection_key = self.version_artifact_cache.selection_key(
            CubeVersionSelectionCacheKey(
                cube_id=context.cube_id,
                version=version,
                source_kind=context.source_kind,
                repo_identity=self._cache_repo_identity(context),
                repo_relative_path=context.repo_relative_path,
                source_revision=self._cache_source_revision(context),
            )
        )
        selection = self.version_artifact_cache.read_selection(selection_key)
        if selection is not None:
            artifact_cache_key = normalize_metadata_string(
                selection.get("artifactCacheKey")
            )
            if artifact_cache_key:
                artifact = self.version_artifact_cache.read_artifact(artifact_cache_key)
                if artifact is not None:
                    _log_cube_library_diagnostic(
                        "sugarcubes_cube_version_cache_hit",
                        cube_id=context.cube_id,
                        version=version,
                        revision_ref=selection.get("revisionRef", ""),
                    )
                    return artifact
                revision_ref = normalize_metadata_string(selection.get("revisionRef"))
                if revision_ref:
                    artifact = self._load_revision_library_artifact(
                        context,
                        revision_ref=revision_ref,
                    )
                    self.version_artifact_cache.write_artifact(
                        artifact_cache_key,
                        artifact,
                    )
                    return artifact
        selected = self._select_newest_historical_cube_version(
            context,
            version=version,
        )
        selected_payload_text = selected.get("_payloadText")
        if isinstance(selected_payload_text, str):
            artifact = self._revision_artifact_from_text(context, selected_payload_text)
        else:
            artifact = self._load_revision_library_artifact(
                context,
                revision_ref=str(selected["revisionRef"]),
            )
        artifact_cache_key = self.version_artifact_cache.artifact_key(
            CubeVersionArtifactCacheKey(
                cube_id=context.cube_id,
                version=version,
                source_kind=context.source_kind,
                repo_identity=self._cache_repo_identity(context),
                repo_relative_path=context.repo_relative_path,
                revision_ref=str(selected["revisionRef"]),
            )
        )
        self.version_artifact_cache.write_selection(
            selection_key,
            revision_ref=str(selected["revisionRef"]),
            content_hash=normalize_metadata_string(artifact.get("contentHash")),
            artifact_cache_key=artifact_cache_key,
        )
        self.version_artifact_cache.write_artifact(artifact_cache_key, artifact)
        return artifact

    def _select_newest_historical_cube_version(
        self,
        context: CubeGitContext,
        *,
        version: str,
    ) -> dict[str, Any]:
        """Return the newest committed ref matching a cube version."""

        if not (context.repo_root / ".git").exists():
            raise BackendError(
                "Cube version was not found.",
                status=404,
                details={"cube_id": context.cube_id, "version": version},
            )
        try:
            result = self.tracked_repo_service.git_runner(
                [
                    "log",
                    "--format=%H%x1f%cI%x1f%s",
                    "--",
                    context.repo_relative_path,
                ],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            message = str(exc)
            if (
                "does not have any commits yet" in message
                or "unknown revision" in message
            ):
                raise BackendError(
                    "Cube version was not found.",
                    status=404,
                    details={"cube_id": context.cube_id, "version": version},
                ) from exc
            raise BackendError("Failed to list cube refs", status=500) from exc

        for line in (getattr(result, "stdout", "") or "").splitlines():
            parts = line.split("\x1f")
            if len(parts) != 3:
                continue
            revision_ref, timestamp, subject = parts
            payload_text = self._git_show_cube(context, revision_ref)
            payload = self._read_revision_payload(payload_text)
            if normalize_metadata_string(payload.get("version")) != version:
                continue
            ref = self._cube_ref_payload(
                context,
                revision_ref=revision_ref,
                payload=payload,
                content_hash=compute_cube_content_hash_bytes(
                    payload_text.encode("utf-8")
                ),
                current=False,
                committed=True,
                label=revision_ref[:7],
                timestamp=timestamp,
                subject=subject,
            )
            ref["_payloadText"] = payload_text
            return ref
        raise BackendError(
            "Cube version was not found.",
            status=404,
            details={"cube_id": context.cube_id, "version": version},
        )

    def _load_revision_library_artifact(
        self,
        context: CubeGitContext,
        *,
        revision_ref: str,
    ) -> dict[str, Any]:
        """Return the backend-facing Cube Library artifact for a git revision."""

        payload_text = self._git_show_cube(context, revision_ref)
        return self._revision_artifact_from_text(context, payload_text)

    def _revision_artifact_from_text(
        self,
        context: CubeGitContext,
        payload_text: str,
    ) -> dict[str, Any]:
        """Return the backend-facing artifact for one git-show payload."""

        payload = self._read_revision_payload(payload_text)
        content_hash = compute_cube_content_hash_bytes(payload_text.encode("utf-8"))
        display_fields = build_cube_identity_fields(
            cube_id=normalize_metadata_string(payload.get("cube_id"))
            or context.cube_id,
            default_alias=derive_cube_display_name(payload, context.cube_path.stem),
            metadata=(
                payload.get("metadata")
                if isinstance(payload.get("metadata"), Mapping)
                else {}
            ),
        )
        metadata = (
            payload.get("metadata")
            if isinstance(payload.get("metadata"), Mapping)
            else {}
        )
        return {
            "schemaVersion": 1,
            "cubeId": normalize_metadata_string(payload.get("cube_id"))
            or context.cube_id,
            "version": normalize_metadata_string(payload.get("version")),
            "displayName": display_fields["display_name"],
            "targetModel": display_fields["target_model"],
            "supportedModels": normalize_supported_models(
                metadata.get("supported_models") if metadata else [],
                target_model=display_fields["target_model"],
            ),
            "contentHash": content_hash,
            "source": self._source_metadata_for_context(context),
            "cube": dict(payload),
        }

    def _cache_repo_identity(self, context: CubeGitContext) -> str:
        """Return stable repository identity for cache partitioning."""

        if context.source_kind == "github":
            return f"{context.owner}/{context.repo}"
        return str(context.repo_root)

    def _cache_source_revision(self, context: CubeGitContext) -> str:
        """Return the source revision fact used to invalidate version selection."""

        if context.source_kind == "github":
            tracked = self.tracked_repo_service.get_repo(context.owner, context.repo)
            return self._local_head_sha(tracked)
        if not (context.repo_root / ".git").exists():
            return "nogit"
        try:
            result = self.tracked_repo_service.git_runner(
                ["rev-parse", "HEAD"],
                cwd=context.repo_root,
            )
        except (OSError, RuntimeError):
            return "unknown"
        return normalize_metadata_string(getattr(result, "stdout", ""))

    def _assert_loaded_artifact_matches_ref(
        self,
        artifact: Mapping[str, Any],
        *,
        cube_id: str,
        revision_ref: str,
        content_hash: str,
        version: str,
    ) -> None:
        """Validate that a loaded artifact satisfies all requested selectors."""

        if normalize_metadata_string(artifact.get("cubeId")) != cube_id:
            raise BackendError(
                "Cube artifact identity mismatch.",
                status=409,
                details={
                    "expected_cube_id": cube_id,
                    "actual_cube_id": normalize_metadata_string(artifact.get("cubeId")),
                },
            )
        if (
            content_hash
            and normalize_metadata_string(artifact.get("contentHash")) != content_hash
        ):
            raise BackendError(
                "Cube artifact content hash mismatch.",
                status=409,
                details={
                    "cube_id": cube_id,
                    "revision_ref": revision_ref,
                    "expected_content_hash": content_hash,
                    "actual_content_hash": normalize_metadata_string(
                        artifact.get("contentHash")
                    ),
                },
            )
        if version and normalize_metadata_string(artifact.get("version")) != version:
            raise BackendError(
                "Cube artifact version mismatch.",
                status=409,
                details={
                    "cube_id": cube_id,
                    "revision_ref": revision_ref,
                    "expected_version": version,
                    "actual_version": normalize_metadata_string(
                        artifact.get("version")
                    ),
                },
            )

    def _read_revision_payload(self, payload_text: str) -> Mapping[str, Any]:
        """Parse one git-show cube payload."""

        try:
            payload = json.loads(payload_text)
        except json.JSONDecodeError as exc:
            raise BackendError(
                "Revision payload is not valid JSON", status=500
            ) from exc
        if not isinstance(payload, Mapping):
            raise BackendError("Revision payload is not a JSON object", status=500)
        return payload

    def _git_show_cube(self, context: CubeGitContext, revision_ref: str) -> str:
        """Read one cube artifact from git history."""

        try:
            result = self.tracked_repo_service.git_runner(
                ["show", f"{revision_ref}:{context.repo_relative_path}"],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            raise BackendError("Failed to load cube revision", status=500) from exc
        return getattr(result, "stdout", "") or ""

    def _source_metadata_for_context(self, context: CubeGitContext) -> dict[str, Any]:
        """Build source metadata directly from resolved cube ownership context."""

        if context.source_kind == "github":
            tracked = self.tracked_repo_service.get_repo(context.owner, context.repo)
            return {
                "kind": "github",
                "repoRef": f"{context.owner}/{context.repo}",
                "owner": context.owner,
                "repo": context.repo,
                "branch": tracked.branch,
                "path": context.repo_relative_path,
                "localHeadSha": self._local_head_sha(tracked),
                "remoteHeadSha": tracked.remote_head_sha,
                "dirty": self._is_repo_path_dirty(
                    context.repo_root, context.repo_relative_path
                ),
            }
        return {
            "kind": "local",
            "namespace": context.namespace,
            "path": context.repo_relative_path,
            "localHeadSha": "",
            "remoteHeadSha": "",
            "dirty": True,
        }

    def resolve_cube_by_id(self, cube_id: str) -> Path:
        """Resolve a source-owned cube path by canonical cube id."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        base_dir = self.resolve_source_base_dir(parsed)
        cube_path = (base_dir / Path(parsed.path)).resolve()
        try:
            cube_path.relative_to(base_dir)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        if not cube_path.exists() or not cube_path.is_file():
            raise BackendError(f"Cube '{normalized}' not found", status=404)
        _log_cube_library_diagnostic(
            "sugarcubes_resolve_cube_by_id",
            cube_id=normalized,
            cube_path=format_display_path(cube_path, self.extension_root),
        )
        return cube_path

    def resolve_cube_target_path(self, cube_id: str) -> Path:
        """Resolve the managed destination path for one canonical cube id."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        base_dir = self.resolve_source_base_dir(parsed)
        target_path = (base_dir / Path(parsed.path)).resolve()
        try:
            target_path.relative_to(base_dir)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        return target_path

    def build_default_alias_lookup(self, cube_ids: Sequence[str]) -> dict[str, str]:
        """Build a cube-id to display-name lookup for export flows."""

        lookup: dict[str, str] = {}
        for cube_id in cube_ids:
            normalized = normalize_metadata_string(cube_id)
            if not normalized:
                continue
            try:
                path = self.resolve_cube_by_id(normalized)
            except BackendError as exc:
                if exc.status == 404:
                    continue
                raise
            payload, error = read_cube_payload(path)
            name = (
                derive_cube_display_name(payload, path.stem)
                if not error
                else normalize_metadata_string(path.stem)
            )
            if not name:
                continue
            lookup[normalized] = name
        return lookup

    def preview_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the lightweight preview payload used by the cube browser."""

        cube_path = self.resolve_cube_by_id(cube_id)
        try:
            loaded_cube = self.load_cube_artifact(cube_path)
            prepared = self.prepare_cube_import(loaded_cube, drop_origin=(0.0, 0.0))
        except CubeImportError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: preview failed for cube '%s'", cube_id)
            raise BackendError("Preview failed", status=500) from exc

        layout = loaded_cube.layout
        collapsed_nodes = 0
        styled_nodes = 0
        if layout:
            for entry in layout.nodes.values():
                if entry.extra.get("collapsed"):
                    collapsed_nodes += 1
                if any(key in entry.extra for key in ("color", "bgcolor", "style")):
                    styled_nodes += 1

        layout_summary = {
            "present": bool(layout),
            "groups": len(layout.groups) if layout else 0,
            "collapsed_nodes": collapsed_nodes,
            "styled_nodes": styled_nodes,
            "ds": layout.ds if layout else {"scale": 1.0, "offset": [0.0, 0.0]},
        }
        stats = {
            "nodes": len(loaded_cube.nodes),
            "markers": len(loaded_cube.markers),
            "inputs": len(loaded_cube.inputs),
            "outputs": len(loaded_cube.outputs),
            "definitions": len(loaded_cube.definitions),
            "prepared_nodes": len(prepared.nodes),
            "prepared_markers": len(prepared.markers),
            "connections": len(prepared.connections),
        }
        warnings = dedupe_warnings(list(loaded_cube.warnings) + list(prepared.warnings))
        source = self.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        icon = attach_icon_url(
            normalize_existing_icon_metadata(loaded_cube.metadata.get("icon")),
            loaded_cube.cube_id,
        )

        log_event(
            "frontend.phase5",
            "preview_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root),
                "node_count": stats["nodes"],
                "marker_count": stats["markers"],
                "layout_present": layout_summary["present"],
            },
        )
        return {
            "cube": {
                "name": cube_path.stem,
                **build_cube_identity_fields(
                    cube_id=loaded_cube.cube_id,
                    default_alias=normalize_metadata_string(
                        loaded_cube.metadata.get("default_alias")
                    )
                    or cube_path.stem,
                    metadata=loaded_cube.metadata,
                ),
                "description": loaded_cube.description,
                "metadata": loaded_cube.metadata,
                "icon": icon,
                "cube_id": loaded_cube.cube_id,
                "version": loaded_cube.version,
            },
            "stats": stats,
            "layout": layout_summary,
            "warnings": warnings,
            "source": {
                "path": format_display_path(cube_path, self.extension_root),
                "relative_path": safe_relative_path(cube_path, base_dir),
                "type": source["source_kind"],
                "owner": source["owner"],
                "repo": source["repo"],
                "repo_ref": source["repo_ref"],
                "namespace": source["namespace"],
            },
        }

    def resolve_cube_icon_asset(self, cube_id: str) -> tuple[Path, str]:
        """Return the resolved icon file and media type for one cube."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' query parameter is required", status=400)

        cube_path = self.resolve_cube_by_id(normalized_cube_id)
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)
        metadata = payload.get("metadata")
        icon_source = metadata.get("icon") if isinstance(metadata, Mapping) else None
        try:
            icon = normalize_icon_metadata(icon_source)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon:
            raise BackendError("Cube icon not found", status=404)

        source = self.resolve_source_descriptor_by_path(cube_path)
        try:
            icon_path = resolve_icon_asset_path(Path(source["base_dir"]), icon)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon_path.exists() or not icon_path.is_file():
            raise BackendError("Cube icon asset not found", status=404)
        return icon_path, icon["media_type"]

    def import_cube_file(
        self,
        *,
        source_value: str,
        target_cube_id: str,
        overwrite: bool,
    ) -> dict[str, Any]:
        """Copy one external `.cube` file into a canonical managed source location."""

        source_path = Path(source_value).expanduser()
        if not source_path.exists() or not source_path.is_file():
            raise BackendError(f"Source cube '{source_value}' not found", status=404)
        if source_path.suffix.lower() != ".cube":
            raise BackendError("Source must be a .cube file", status=400)

        normalized_target_cube_id = normalize_metadata_string(target_cube_id)
        if not normalized_target_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized_target_cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        self.ownership_policy_service.assert_cube_id_writable(
            normalized_target_cube_id,
            action="import a cube into that destination",
        )

        resolved_dir = self.resolve_source_base_dir(parsed)
        resolved_dir.mkdir(parents=True, exist_ok=True)
        dest_path = (resolved_dir / Path(parsed.path)).resolve()
        try:
            dest_path.relative_to(resolved_dir)
        except ValueError as exc:
            raise BackendError("Invalid destination", status=400) from exc
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        if dest_path.exists() and not overwrite:
            raise BackendError(
                f"Cube '{normalized_target_cube_id}' already exists", status=409
            )

        try:
            shutil.copy2(source_path, dest_path)
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to import cube from %s to %s",
                source_path,
                dest_path,
            )
            raise BackendError("Failed to import cube", status=500) from exc

        payload, error = read_cube_payload(dest_path)
        if payload and not error:
            payload_dict = dict(payload)
            previous_cube_id = normalize_metadata_string(payload_dict.get("cube_id"))
            payload_dict["cube_id"] = normalized_target_cube_id
            try:
                apply_cube_identity_projection(
                    payload_dict, previous_cube_id=previous_cube_id
                )
                with dest_path.open("w", encoding="utf-8") as handle:
                    json.dump(payload_dict, handle, indent=2)
                    handle.write("\n")
            except (OSError, TypeError, ValueError) as exc:
                _cleanup_failed_import(dest_path)
                _logger.exception(
                    "SugarCubes: failed to persist imported cube identity for %s",
                    dest_path,
                )
                raise BackendError("Failed to import cube", status=500) from exc

        try:
            self.load_cube_artifact(dest_path)
        except CubeImportError:
            _cleanup_failed_import(dest_path)
            raise
        except Exception as exc:
            _cleanup_failed_import(dest_path)
            _logger.exception(
                "SugarCubes: imported cube failed validation for %s",
                dest_path,
            )
            raise BackendError("Imported cube failed validation", status=500) from exc

        log_event(
            "frontend.phase5",
            "import_cube_file",
            {
                "source": str(source_path.name),
                "dest": safe_relative_path(dest_path, resolved_dir)
                or normalized_target_cube_id,
            },
        )
        self.invalidate_catalog_state(
            reason="cube_imported",
            affected_cube_ids=[normalized_target_cube_id],
        )
        return {"cube": self.summarize_cube(dest_path)}

    def delete_cube(
        self,
        *,
        cube_id: str,
    ) -> dict[str, Any]:
        """Delete a tracked cube by canonical id."""

        self.ownership_policy_service.assert_cube_id_writable(
            cube_id,
            action="delete this cube",
        )
        cube_path = self.resolve_cube_by_id(cube_id)

        try:
            cube_path.unlink()
        except FileNotFoundError as exc:
            raise BackendError("Cube already removed", status=404) from exc
        except OSError as exc:
            _logger.exception("SugarCubes: failed to delete cube %s", cube_path)
            raise BackendError("Failed to delete cube", status=500) from exc

        source = self.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        log_event(
            "frontend.phase5",
            "delete_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root)
            },
        )
        self.invalidate_catalog_state(
            reason="cube_deleted", affected_cube_ids=[cube_id]
        )
        return {
            "status": "deleted",
            "cube": format_display_path(cube_path, self.extension_root),
        }

    def _list_repo_cubes(
        self,
        tracked: TrackedRepo,
        *,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List all cube files under one tracked repo checkout."""

        started_at = perf_counter()
        phase_started_at = started_at

        def record_phase(name: str) -> None:
            """Accumulate elapsed milliseconds for one repo cube listing phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round(
                phase_timings.get(name, 0.0) + ((now - phase_started_at) * 1000),
                3,
            )
            phase_started_at = now

        phase_timings: dict[str, float] = {}
        checkout_path = Path(tracked.local_checkout_path).resolve()
        if not checkout_path.exists():
            return []
        cube_files = [
            path for path in list_cube_files(checkout_path) if ".git" not in path.parts
        ]
        record_phase("list_cube_files")
        cubes: list[dict[str, Any]] = []
        for path in cube_files:
            summary = summarize_cube_file(
                path,
                checkout_path,
                self.extension_root,
                source_kind="github",
                owner=tracked.owner,
                repo=tracked.repo,
                include_internal_payload=include_internal_payload,
            )
            record_phase("summarize_cube_file")
            cubes.append(self.ownership_policy_service.annotate_cube_payload(summary))
            record_phase("annotate_cube_payload")
        _log_cube_library_diagnostic(
            "sugarcubes_repo_cube_listing_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            repo_ref=tracked.repo_ref,
            cube_count=len(cubes),
            include_internal_payload=include_internal_payload,
            **phase_timings,
        )
        return cubes

    def _list_local_cubes(
        self,
        *,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List all cube files under the managed local source workspace."""

        workspace_root = self.local_workspace_root().resolve()
        if not workspace_root.exists():
            return []
        cubes: list[dict[str, Any]] = []
        namespace_dirs = [path for path in workspace_root.iterdir() if path.is_dir()]
        for namespace_dir in sorted(namespace_dirs):
            namespace = namespace_dir.name
            if namespace.lower() in RESERVED_SOURCE_NAMES:
                continue
            for path in list_cube_files(namespace_dir):
                cubes.append(
                    self.ownership_policy_service.annotate_cube_payload(
                        summarize_cube_file(
                            path,
                            namespace_dir,
                            self.extension_root,
                            source_kind="local",
                            namespace=namespace,
                            include_internal_payload=include_internal_payload,
                        )
                    )
                )
        return cubes

    def _list_catalog_cube_summaries(
        self,
        *,
        include_disabled: bool,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List cube summaries for the backend-facing Cube Library catalog."""

        started_at = perf_counter()
        phase_started_at = started_at
        phase_timings: dict[str, float] = {}

        def record_phase(name: str) -> None:
            """Record elapsed milliseconds for one catalog summary listing phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round((now - phase_started_at) * 1000, 3)
            phase_started_at = now

        cubes: list[dict[str, Any]] = []
        repo_entries = self.tracked_repo_service.list_repos()["repos"]
        record_phase("list_repos")
        enabled_repo_count = 0
        repo_cube_count = 0
        for repo_entry in repo_entries:
            if not include_disabled and not repo_entry.get("enabled"):
                continue
            enabled_repo_count += 1
            tracked = self._tracked_repo_from_payload(repo_entry)
            repo_cubes = self._list_repo_cubes(
                tracked,
                include_internal_payload=include_internal_payload,
            )
            repo_cube_count += len(repo_cubes)
            cubes.extend(repo_cubes)
        record_phase("list_repo_cubes")
        local_cubes = self._list_local_cubes(
            include_internal_payload=include_internal_payload
        )
        record_phase("list_local_cubes")
        cubes.extend(local_cubes)
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_summary_listing_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            include_disabled=include_disabled,
            include_internal_payload=include_internal_payload,
            repo_count=len(repo_entries),
            enabled_repo_count=enabled_repo_count,
            repo_cube_count=repo_cube_count,
            local_cube_count=len(local_cubes),
            total_cube_count=len(cubes),
            **phase_timings,
        )
        return cubes

    def _catalog_entry_for_summary(self, summary: Mapping[str, Any]) -> dict[str, Any]:
        """Convert a browser cube summary into a backend catalog entry."""

        cube_id = normalize_metadata_string(summary.get("cube_id"))
        payload, _, content_hash = self._summary_payload_with_hash(summary)
        icon = summary.get("icon") if isinstance(summary.get("icon"), Mapping) else None
        entry: dict[str, Any] = {
            "cubeId": cube_id,
            "displayName": normalize_metadata_string(summary.get("display_name"))
            or normalize_metadata_string(summary.get("name")),
            "version": normalize_metadata_string(summary.get("version")),
            "description": normalize_metadata_string(summary.get("description")),
            "targetModel": normalize_metadata_string(summary.get("target_model")),
            "supportedModels": list(summary.get("supported_models") or []),
            "source": self._source_metadata_for_summary(summary),
            "contentHash": content_hash,
            "updatedAt": normalize_metadata_string(summary.get("mtime")),
        }
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_entry",
            cube_id=cube_id,
            version=entry["version"],
            content_hash=entry["contentHash"],
            source_kind=(
                entry["source"].get("kind", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
            source_path=(
                entry["source"].get("path", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
        )
        if icon:
            entry["icon"] = dict(icon)
        if payload:
            requirements = iter_custom_node_slugs(payload)
            if requirements:
                entry["requiredCustomNodes"] = list(requirements)
        return entry

    def _summary_payload_with_hash(
        self,
        summary: Mapping[str, Any],
    ) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]:
        """Return payload and hash from internal summary facts or a fallback read."""

        if "_content_hash" in summary:
            payload = summary.get("_payload")
            return (
                dict(payload) if isinstance(payload, Mapping) else None,
                normalize_metadata_string(summary.get("error")) or None,
                normalize_metadata_string(summary.get("_content_hash")),
            )
        cube_id = normalize_metadata_string(summary.get("cube_id"))
        cube_path = self.resolve_cube_by_id(cube_id)
        return read_cube_payload_with_hash(cube_path)

    def _source_metadata_for_summary(
        self,
        summary: Mapping[str, Any],
        *,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None = None,
    ) -> dict[str, Any]:
        """Build API source metadata for one summarized cube."""

        cube_id = normalize_metadata_string(summary.get("cube_id"))
        source = (
            summary.get("source") if isinstance(summary.get("source"), Mapping) else {}
        )
        source_kind = normalize_metadata_string(
            source.get("type")
        ) or normalize_metadata_string(summary.get("source_kind"))
        if source_kind == "github":
            owner = normalize_metadata_string(
                source.get("owner") or summary.get("owner")
            )
            repo = normalize_metadata_string(source.get("repo") or summary.get("repo"))
            tracked = self._tracked_repo_for_source(
                owner=owner,
                repo=repo,
                repo_cache=repo_cache,
            )
            base_dir = Path(tracked.local_checkout_path).resolve()
            relative_path = normalize_metadata_string(
                source.get("repo_relative_path") or summary.get("relative_path")
            )
            return {
                "kind": "github",
                "repoRef": f"{owner}/{repo}",
                "owner": owner,
                "repo": repo,
                "branch": tracked.branch,
                "path": relative_path,
                "localHeadSha": self._local_head_sha(tracked),
                "remoteHeadSha": tracked.remote_head_sha,
                "dirty": self._is_repo_path_dirty(base_dir, relative_path),
            }
        namespace = normalize_metadata_string(
            source.get("namespace") or summary.get("namespace")
        )
        return {
            "kind": "local",
            "namespace": namespace,
            "path": self._local_source_relative_path(cube_id),
            "localHeadSha": "",
            "remoteHeadSha": "",
            "dirty": True,
        }

    def _tracked_repo_for_source(
        self,
        *,
        owner: str,
        repo: str,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None,
    ) -> TrackedRepo:
        """Return tracked repo facts, reusing manifest lookups within one pass."""

        if repo_cache is None:
            return self.tracked_repo_service.get_repo(owner, repo)
        cache_key = (owner.casefold(), repo.casefold())
        cached = repo_cache.get(cache_key)
        if cached is not None:
            return cached
        tracked = self.tracked_repo_service.get_repo(owner, repo)
        repo_cache[cache_key] = tracked
        return tracked

    def _pack_record(self, repo_entry: Mapping[str, Any]) -> dict[str, Any]:
        """Return an API-safe Cube Pack record from SugarCubes repo state."""

        owner = normalize_metadata_string(repo_entry.get("owner"))
        repo = normalize_metadata_string(repo_entry.get("repo"))
        repo_ref = (
            normalize_metadata_string(repo_entry.get("repo_ref")) or f"{owner}/{repo}"
        )
        checkout_path_raw = normalize_metadata_string(
            repo_entry.get("local_checkout_path")
        )
        checkout_path = (
            Path(checkout_path_raw) if checkout_path_raw else Path("__missing__")
        )
        return {
            "repoRef": repo_ref,
            "owner": owner,
            "repo": repo,
            "branch": normalize_metadata_string(repo_entry.get("branch")) or "main",
            "enabled": bool(repo_entry.get("enabled")),
            "defaultBaseRepo": bool(repo_entry.get("default_base_repo")),
            "autoUpdate": bool(repo_entry.get("auto_update")),
            "localHeadSha": normalize_metadata_string(repo_entry.get("local_head_sha")),
            "remoteHeadSha": normalize_metadata_string(
                repo_entry.get("remote_head_sha")
            ),
            "updateAvailable": bool(repo_entry.get("update_available")),
            "lastSyncAt": normalize_metadata_string(repo_entry.get("last_sync_at")),
            "lastSyncStatus": normalize_metadata_string(
                repo_entry.get("last_sync_status")
            )
            or "never",
            "lastSyncError": normalize_metadata_string(
                repo_entry.get("last_sync_error")
            ),
            "lastCheckedAt": normalize_metadata_string(
                repo_entry.get("last_checked_at")
            ),
            "lastCheckStatus": normalize_metadata_string(
                repo_entry.get("last_check_status")
            )
            or "never",
            "lastCheckError": normalize_metadata_string(
                repo_entry.get("last_check_error")
            ),
            "cubeCount": self._count_cube_files(checkout_path),
        }

    def _pack_counts(self) -> dict[str, int]:
        """Return count metadata for tracked Cube Packs."""

        repos = self.tracked_repo_service.list_repos()["repos"]
        return {
            "count": len(repos),
            "enabledCount": sum(1 for repo in repos if repo.get("enabled")),
        }

    def _revision_pack_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]:
        """Return normalized pack facts used to compute catalog revisions."""

        facts: list[dict[str, Any]] = []
        for repo in self.tracked_repo_service.list_repos()["repos"]:
            if not include_disabled and not repo.get("enabled"):
                continue
            facts.append(
                {
                    "repo_ref": repo.get("repo_ref"),
                    "branch": repo.get("branch"),
                    "enabled": bool(repo.get("enabled")),
                    "local_head_sha": repo.get("local_head_sha"),
                    "remote_head_sha": repo.get("remote_head_sha"),
                    "update_available": bool(repo.get("update_available")),
                }
            )
        return sorted(facts, key=lambda fact: str(fact.get("repo_ref", "")).casefold())

    def _revision_cube_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]:
        """Return normalized cube facts used to compute catalog revisions."""

        facts: list[dict[str, Any]] = []
        repo_cache: dict[tuple[str, str], TrackedRepo] = {}
        for summary in self._list_catalog_cube_summaries(
            include_disabled=include_disabled,
            include_internal_payload=True,
        ):
            cube_id = normalize_metadata_string(summary.get("cube_id"))
            _, _, content_hash = self._summary_payload_with_hash(summary)
            source = self._source_metadata_for_summary(summary, repo_cache=repo_cache)
            facts.append(
                {
                    "cube_id": cube_id,
                    "version": normalize_metadata_string(summary.get("version")),
                    "content_hash": content_hash,
                    "source": source,
                }
            )
        return sorted(facts, key=lambda fact: str(fact.get("cube_id", "")).casefold())

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
            _log_cube_library_diagnostic(
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
        summaries = self._list_catalog_cube_summaries(
            include_disabled=False,
            include_internal_payload=True,
        )
        add_phase_time("list_catalog_cube_summaries", phase_started_at)
        skipped_payload_count = 0
        for summary in summaries:
            cube_id = normalize_metadata_string(summary.get("cube_id"))
            try:
                phase_started_at = perf_counter()
                payload, error, content_hash = self._summary_payload_with_hash(summary)
                add_phase_time("summary_payload_with_hash", phase_started_at)
            except BackendError:
                skipped_payload_count += 1
                continue
            phase_started_at = perf_counter()
            source = self._source_metadata_for_summary(summary, repo_cache=repo_cache)
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
        _log_cube_library_diagnostic(
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

        repo_entries = self.tracked_repo_service.list_repos()["repos"]
        repo_cube_facts: list[dict[str, Any]] = []
        for repo_entry in repo_entries:
            if not repo_entry.get("enabled"):
                continue
            tracked = self._tracked_repo_from_payload(repo_entry)
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
        local_root = self.local_workspace_root().resolve()
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
            "packs": self._revision_pack_facts(include_disabled=False),
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
            self.extension_root
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
            "packs": self._revision_pack_facts(include_disabled=False),
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

        source = (
            summary.get("source") if isinstance(summary.get("source"), Mapping) else {}
        )
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
            _normalize_requirement_key(name): name for name in installed
        }
        by_node: dict[str, dict[str, Any]] = {}
        for record in requirement_records:
            node_id = normalize_metadata_string(record.get("node_id"))
            if not node_id:
                continue
            key = _normalize_requirement_key(node_id)
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

    def _local_source_relative_path(self, cube_id: str) -> str:
        """Return the source-relative path for a local canonical cube id."""

        try:
            parsed = parse_canonical_cube_id(cube_id)
        except CubeIdentityError:
            return ""
        return parsed.path if parsed.source_kind == "local" else ""

    def _local_head_sha(self, tracked: TrackedRepo) -> str:
        """Return persisted or live HEAD SHA for a tracked checkout."""

        if tracked.local_head_sha:
            return tracked.local_head_sha
        checkout = Path(tracked.local_checkout_path).resolve()
        if not (checkout / ".git").exists():
            return ""
        try:
            result = self.tracked_repo_service.git_runner(
                ["rev-parse", "HEAD"], cwd=checkout
            )
        except (OSError, RuntimeError):
            return ""
        return normalize_metadata_string(getattr(result, "stdout", ""))

    def _is_repo_path_dirty(self, checkout: Path, relative_path: str) -> bool:
        """Return whether a repo-relative cube artifact differs from clean HEAD."""

        if not relative_path or not (checkout / ".git").exists():
            return False
        return relative_path.replace("\\", "/") in self._repo_dirty_paths(checkout)

    def _repo_dirty_paths(self, checkout: Path) -> frozenset[str]:
        """Return dirty repo paths from one cached git status scan."""

        started_at = perf_counter()
        checkout = checkout.resolve()
        cached = self._repo_dirty_paths_cache.get(checkout)
        if cached is not None:
            _log_cube_library_diagnostic(
                "sugarcubes_repo_dirty_paths_timing",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
                checkout=checkout.name,
                cached=True,
                dirty_path_count=len(cached),
            )
            return cached
        try:
            result = self.tracked_repo_service.git_runner(
                ["status", "--porcelain"],
                cwd=checkout,
            )
        except (OSError, RuntimeError):
            dirty_paths: frozenset[str] = frozenset()
        else:
            dirty_paths = frozenset(
                _git_status_path(line)
                for line in str(getattr(result, "stdout", "")).splitlines()
                if _git_status_path(line)
            )
        self._repo_dirty_paths_cache[checkout] = dirty_paths
        _log_cube_library_diagnostic(
            "sugarcubes_repo_dirty_paths_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            checkout=checkout.name,
            cached=False,
            dirty_path_count=len(dirty_paths),
        )
        return dirty_paths

    def _count_cube_files(self, root: Path) -> int:
        """Return the number of loadable cube files under one checkout path."""

        if not root.exists() or not root.is_dir():
            return 0
        return len([path for path in list_cube_files(root) if ".git" not in path.parts])

    def _tracked_repo_from_payload(self, repo_entry: Mapping[str, Any]) -> TrackedRepo:
        """Convert serialized manifest state back into a tracked repo record."""

        return TrackedRepo(
            owner=str(repo_entry["owner"]),
            repo=str(repo_entry["repo"]),
            branch=str(repo_entry["branch"]),
            enabled=bool(repo_entry["enabled"]),
            default_base_repo=bool(repo_entry["default_base_repo"]),
            auto_update=bool(repo_entry.get("auto_update", False)),
            local_checkout_path=str(repo_entry["local_checkout_path"]),
            last_sync_at=str(repo_entry["last_sync_at"]),
            last_sync_status=str(repo_entry["last_sync_status"]),
            last_sync_error=str(repo_entry["last_sync_error"]),
            last_checked_at=str(repo_entry.get("last_checked_at") or ""),
            last_check_status=str(repo_entry.get("last_check_status") or "never"),
            last_check_error=str(repo_entry.get("last_check_error") or ""),
            remote_head_sha=str(repo_entry.get("remote_head_sha") or ""),
            local_head_sha=str(repo_entry.get("local_head_sha") or ""),
            update_available=bool(repo_entry.get("update_available", False)),
        )

    def resolve_source_base_dir(self, parsed_cube_id: Any) -> Path:
        """Resolve the managed base directory for one parsed canonical cube id."""

        if parsed_cube_id.source_kind == "github":
            tracked = self.tracked_repo_service.get_repo(
                parsed_cube_id.owner, parsed_cube_id.repo
            )
            return Path(
                tracked.local_checkout_path
                or self.tracked_repo_service.checkout_path(
                    parsed_cube_id.owner, parsed_cube_id.repo
                )
            ).resolve()
        local_root = self.local_workspace_root().resolve()
        namespace_root = (local_root / parsed_cube_id.namespace).resolve()
        try:
            namespace_root.relative_to(local_root)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        return namespace_root

    def resolve_source_descriptor_by_path(self, cube_path: Path) -> dict[str, str]:
        """Resolve source ownership metadata for one local cube path."""

        resolved_path = cube_path.resolve()
        for repo_entry in self.tracked_repo_service.list_repos()["repos"]:
            checkout_path = Path(repo_entry["local_checkout_path"]).resolve()
            try:
                resolved_path.relative_to(checkout_path)
            except ValueError:
                continue
            return {
                "source_kind": "github",
                "base_dir": str(checkout_path),
                "owner": repo_entry["owner"],
                "repo": repo_entry["repo"],
                "repo_ref": f"{repo_entry['owner']}/{repo_entry['repo']}",
                "namespace": "",
            }

        local_root = self.local_workspace_root().resolve()
        try:
            relative_path = resolved_path.relative_to(local_root)
        except ValueError as exc:
            raise BackendError(
                "Cube path is not owned by a managed source", status=404
            ) from exc
        if not relative_path.parts:
            raise BackendError("Cube path is not owned by a managed source", status=404)
        namespace = relative_path.parts[0]
        return {
            "source_kind": "local",
            "base_dir": str((local_root / namespace).resolve()),
            "owner": "",
            "repo": "",
            "repo_ref": "",
            "namespace": namespace,
        }

    def _resolve_tracked_repo_for_path(self, cube_path: Path) -> TrackedRepo:
        """Resolve which tracked repo owns one local cube path."""

        resolved_path = cube_path.resolve()
        for repo_entry in self.tracked_repo_service.list_repos()["repos"]:
            checkout_path = Path(repo_entry["local_checkout_path"]).resolve()
            try:
                resolved_path.relative_to(checkout_path)
            except ValueError:
                continue
            return TrackedRepo(
                owner=repo_entry["owner"],
                repo=repo_entry["repo"],
                branch=repo_entry["branch"],
                enabled=bool(repo_entry["enabled"]),
                default_base_repo=bool(repo_entry["default_base_repo"]),
                local_checkout_path=repo_entry["local_checkout_path"],
                last_sync_at=repo_entry["last_sync_at"],
                last_sync_status=repo_entry["last_sync_status"],
                last_sync_error=repo_entry["last_sync_error"],
            )
        raise BackendError("Cube path is not owned by a tracked repo", status=404)
