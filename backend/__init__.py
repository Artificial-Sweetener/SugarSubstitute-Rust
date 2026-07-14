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
"""Backend composition helpers for SugarCubes."""

from __future__ import annotations

import importlib
import importlib.metadata
import logging
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, Optional

try:
    from ..exporter import (
        export as export_cubes,
        write_cube,
        write_cube_to_path,
        write_cubes,
        write_cubes_to_paths,
    )
    from ..exporter.versioning import suggest_version
    from ..importer import (
        load_cube as load_cube_artifact,
        prepare_import as prepare_cube_import,
    )
    from ..nodes import NODE_CLASS_MAPPINGS as SUGAR_NODE_CLASS_MAPPINGS
    from ..payloads import retarget_cube_payload
    from .services import (
        CubeArtifactRepository,
        CubeExportService,
        CubeDependencyService,
        CubeHistoryService,
        CubeIdentityRedirectService,
        CubeLibraryService,
        CubeLoadService,
        CubeMetadataService,
        CubePromotionService,
        CubeRevisionService,
        IdentityPolicyService,
        LocalFlavorService,
        OwnershipPolicyService,
        TrackedRepoService,
    )
except ImportError:
    from exporter import (
        export as export_cubes,
        write_cube,
        write_cube_to_path,
        write_cubes,
        write_cubes_to_paths,
    )
    from exporter.versioning import suggest_version
    from importer import (
        load_cube as load_cube_artifact,
        prepare_import as prepare_cube_import,
    )
    from nodes import NODE_CLASS_MAPPINGS as SUGAR_NODE_CLASS_MAPPINGS
    from payloads import retarget_cube_payload
    from backend.services import (
        CubeArtifactRepository,
        CubeExportService,
        CubeDependencyService,
        CubeHistoryService,
        CubeIdentityRedirectService,
        CubeLibraryService,
        CubeLoadService,
        CubeMetadataService,
        CubePromotionService,
        CubeRevisionService,
        IdentityPolicyService,
        LocalFlavorService,
        OwnershipPolicyService,
        TrackedRepoService,
    )

_logger = logging.getLogger(__name__)
_DISTRIBUTION_NAME = "SugarCubes"


def _runtime_version() -> str:
    """Return the installed SugarCubes version from canonical project metadata."""

    pyproject_path = Path(__file__).resolve().parents[1] / "pyproject.toml"
    if pyproject_path.exists():
        metadata = tomllib.loads(pyproject_path.read_text(encoding="utf-8"))
        version = metadata.get("project", {}).get("version")
        if isinstance(version, str) and version.strip():
            return version
        raise RuntimeError("SugarCubes pyproject.toml does not define a version.")
    try:
        return importlib.metadata.version(_DISTRIBUTION_NAME)
    except importlib.metadata.PackageNotFoundError:
        raise RuntimeError("SugarCubes package metadata is unavailable.") from None


__version__ = _runtime_version()


@dataclass(frozen=True)
class BackendServices:
    """Concrete service bundle used by the backend route layer."""

    library: CubeLibraryService
    tracked_repos: TrackedRepoService
    identity: IdentityPolicyService
    ownership: OwnershipPolicyService
    metadata: CubeMetadataService
    promotion: CubePromotionService
    redirects: CubeIdentityRedirectService
    exporter: CubeExportService
    loader: CubeLoadService
    revisions: CubeRevisionService
    local_flavors: LocalFlavorService
    dependencies: CubeDependencyService


_ACTIVE_BACKEND_SERVICES: BackendServices | None = None


def set_active_backend_services(services: BackendServices) -> None:
    """Store the active SugarCubes service graph for host adapters."""

    global _ACTIVE_BACKEND_SERVICES
    _ACTIVE_BACKEND_SERVICES = services


def active_backend_services() -> BackendServices | None:
    """Return the active SugarCubes service graph when Comfy registered it."""

    return _ACTIVE_BACKEND_SERVICES


def _coerce_node_class_mappings(module: Any) -> Optional[Mapping[str, Any]]:
    """Return `NODE_CLASS_MAPPINGS` when the module exposes a mapping."""

    mapping = getattr(module, "NODE_CLASS_MAPPINGS", None)
    return mapping if isinstance(mapping, Mapping) else None


def _is_extension_nodes_module(module: Any, extension_root: Path) -> bool:
    """Return whether the module resolves to SugarCubes' local `nodes.py`."""

    module_file = getattr(module, "__file__", None)
    if not isinstance(module_file, str):
        return False
    try:
        return Path(module_file).resolve() == (extension_root / "nodes.py").resolve()
    except OSError:
        return False


def _load_active_comfy_nodes_module(extension_root: Path) -> Optional[Any]:
    """Load the host Comfy `nodes` module without accepting local shadowing."""

    existing_module = sys.modules.get("nodes")
    if existing_module is not None and not _is_extension_nodes_module(
        existing_module, extension_root
    ):
        return existing_module

    try:
        comfy_nodes_module = importlib.import_module("nodes")
    except (ImportError, ModuleNotFoundError):
        return None

    if _is_extension_nodes_module(comfy_nodes_module, extension_root):
        return None
    return comfy_nodes_module


def _resolve_active_comfy_node_class_mappings(
    extension_root: Path,
) -> Mapping[str, Any]:
    """Return the live Comfy node registry merged with SugarCubes marker nodes."""

    comfy_nodes_module = _load_active_comfy_nodes_module(extension_root)
    comfy_node_class_mappings = (
        _coerce_node_class_mappings(comfy_nodes_module)
        if comfy_nodes_module is not None
        else None
    )
    if comfy_node_class_mappings is None:
        _logger.warning(
            "SugarCubes: active Comfy nodes registry unavailable; export validation "
            "is falling back to SugarCubes marker nodes only"
        )
        return dict(SUGAR_NODE_CLASS_MAPPINGS)

    merged_mappings = dict(comfy_node_class_mappings)
    merged_mappings.update(SUGAR_NODE_CLASS_MAPPINGS)
    return merged_mappings


def build_backend_services(
    extension_root: Path,
    *,
    workspace_path: Path | None = None,
    custom_nodes_root: Path | None = None,
) -> BackendServices:
    """Build the repository-standard backend service graph."""

    identity = IdentityPolicyService(extension_root)
    tracked_repos = TrackedRepoService(
        extension_root,
        protected_owner_provider=lambda: identity.get_policy().claimed_github_owner,
    )
    local_flavors = LocalFlavorService(tracked_repos)
    artifacts = CubeArtifactRepository(tracked_repos)
    history = CubeHistoryService(tracked_repos)
    redirects = CubeIdentityRedirectService(tracked_repos)
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )
    library = CubeLibraryService(
        extension_root,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        tracked_repo_service=tracked_repos,
        ownership_policy_service=ownership,
        registry_factory=None,
    )
    metadata = CubeMetadataService(
        library,
        retarget_cube_payload=retarget_cube_payload,
        artifacts=artifacts,
        history=history,
        local_flavors=local_flavors,
    )
    promotion = CubePromotionService(
        artifacts=artifacts,
        history=history,
        redirects=redirects,
        local_flavors=local_flavors,
        ownership=ownership,
        library=library,
        retarget_cube_payload=retarget_cube_payload,
    )
    loader = CubeLoadService(
        library,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        redirect_service=redirects,
    )
    exporter = CubeExportService(
        library,
        export_cubes=export_cubes,
        write_cube=write_cube,
        write_cubes=write_cubes,
        write_cube_to_path=write_cube_to_path,
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=suggest_version,
        node_class_mappings_provider=lambda: _resolve_active_comfy_node_class_mappings(
            extension_root
        ),
        finalized_definition_provider=lambda path, cube_id, _payload: loader.load_cube_path(
            cube_path=path,
            cube_id=cube_id,
        ),
        local_flavor_service=local_flavors,
    )
    revisions = CubeRevisionService(
        library,
        tracked_repos,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        redirect_service=redirects,
    )
    dependencies = CubeDependencyService(
        library_service=library,
        tracked_repo_service=tracked_repos,
        workspace_path=workspace_path or extension_root.parent.parent,
        custom_nodes_root=custom_nodes_root or extension_root.parent,
    )
    return BackendServices(
        library=library,
        tracked_repos=tracked_repos,
        identity=identity,
        ownership=ownership,
        metadata=metadata,
        promotion=promotion,
        redirects=redirects,
        exporter=exporter,
        loader=loader,
        revisions=revisions,
        local_flavors=local_flavors,
        dependencies=dependencies,
    )


__all__ = [
    "BackendServices",
    "active_backend_services",
    "build_backend_services",
    "set_active_backend_services",
]
