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
"""Build stable API summaries from cube artifact documents."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

from ...cube_model import (
    CubeIdentityError,
    derive_route_from_cube_id,
    derive_target_model_from_cube_id,
    looks_like_current_cube_payload,
    normalize_target_model,
)
from .cube_file_io import (
    format_display_path,
    format_timestamp,
    read_cube_payload,
    read_cube_payload_with_hash as read_cube_payload_with_hash,
    safe_relative_path,
)
from .cube_icon_service import attach_icon_url, normalize_existing_icon_metadata
from .cube_metadata import (
    derive_source_author_from_identity,
    normalize_lineage_payload,
    normalize_metadata_string,
    normalize_supported_models,
    normalize_tags,
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
    cube_id = ""
    version = ""
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

    entry: dict[str, Any] = {
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
    """Return the layout mapping for either current or legacy payloads."""

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
