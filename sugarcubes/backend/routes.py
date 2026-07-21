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
"""PromptServer route registration for SugarCubes backend APIs."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Callable, Coroutine, Mapping

from aiohttp import web

from ..importer import CubeImportError
from .composition import BackendServices
from .responses import (
    BackendError,
    json_error,
    json_error_from_exception,
    json_success,
)
from .services.cube_metadata import normalize_metadata_string
from .services.cube_dependency_service import DependencyApprovalPolicy
from .validation import (
    coerce_int,
    extract_drop_origin,
    get_bool,
    normalize_actor,
    normalize_graph_payload,
    normalize_workflow_payload,
    parse_json_body,
    parse_optional_json_body,
    parse_save_many_cube_entries,
)

_logger = logging.getLogger(__name__)
RouteHandler = Callable[[Any], Coroutine[Any, Any, Any]]


@dataclass(frozen=True)
class RouteHandlers:
    """Concrete route callables used for registration and tests."""

    list_cubes: RouteHandler
    get_identity_policy: RouteHandler
    update_identity_policy: RouteHandler
    list_tracked_repos: RouteHandler
    preflight_tracked_repo: RouteHandler
    add_tracked_repo: RouteHandler
    create_authoring_repo: RouteHandler
    update_tracked_repo: RouteHandler
    remove_tracked_repo: RouteHandler
    sync_tracked_repo: RouteHandler
    sync_all_tracked_repos: RouteHandler
    check_tracked_repo: RouteHandler
    check_all_tracked_repos: RouteHandler
    list_revisions: RouteHandler
    load_revision: RouteHandler
    preview_cube: RouteHandler
    serve_icon_asset: RouteHandler
    load_cube: RouteHandler
    update_metadata: RouteHandler
    rename_cube: RouteHandler
    promote_cube: RouteHandler
    delete_cube: RouteHandler
    import_cube_file: RouteHandler
    save_many: RouteHandler
    save_implementation: RouteHandler
    save_authored_flavor: RouteHandler
    get_local_flavors: RouteHandler
    save_local_flavor: RouteHandler
    delete_local_flavor: RouteHandler
    select_local_flavor: RouteHandler
    migrate_local_flavors: RouteHandler
    reconcile_local_flavors: RouteHandler
    get_dependency_readiness: RouteHandler
    repair_dependencies: RouteHandler
    sync_and_check_dependencies: RouteHandler


def build_route_handlers(services: BackendServices) -> RouteHandlers:
    """Build thin HTTP handlers over the backend services."""

    async def list_cubes(request: Any) -> Any:
        _ = request
        try:
            return json_success(services.library.list_cubes(), status=200)
        except BackendError as error:
            return json_error_from_exception(error)
        except Exception:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: failed to list cubes")
            return json_error("Failed to list SugarCubes", status=500)

    async def list_tracked_repos(request: Any) -> Any:
        _ = request
        try:
            return json_success(
                services.ownership.annotate_repo_list_payload(
                    services.tracked_repos.list_repos()
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def preflight_tracked_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            payload = services.tracked_repos.preflight_repo(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
                branch=normalize_metadata_string(body.get("branch")) or "main",
            )
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def get_identity_policy(request: Any) -> Any:
        _ = request
        try:
            return json_success(services.ownership.list_identity_policy(), status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def update_identity_policy(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            if "allow_system_owner_claim" in body:
                raise BackendError(
                    "allow_system_owner_claim is managed only by environment configuration",
                    status=400,
                )
            payload = services.ownership.update_identity_policy(
                claimed_github_owner=(
                    str(body.get("claimed_github_owner"))
                    if "claimed_github_owner" in body
                    else None
                ),
            )
            services.library.invalidate_catalog_state(reason="identity_policy_updated")
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def add_tracked_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            payload = services.tracked_repos.add_repo(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
                branch=normalize_metadata_string(body.get("branch")) or "main",
                enabled=get_bool(body, "enabled", True),
                default_base_repo=False,
                auto_update=get_bool(body, "auto_update", False),
            )
            services.library.invalidate_catalog_state(reason="pack_added")
            return json_success(
                {
                    **payload,
                    "repo": services.ownership.annotate_repo_payload(payload["repo"]),
                },
                status=201,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def create_authoring_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            owner, repo = services.ownership.assert_authoring_repo_allowed(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
            )
            payload = services.tracked_repos.ensure_authoring_repo(
                owner=owner,
                repo=repo,
                branch=normalize_metadata_string(body.get("branch")) or "main",
            )
            services.library.invalidate_catalog_state(reason="authoring_pack_ensured")
            return json_success(
                {
                    **payload,
                    "repo": services.ownership.annotate_repo_payload(payload["repo"]),
                },
                status=201,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def update_tracked_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            payload = services.tracked_repos.update_repo(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
                branch=(
                    normalize_metadata_string(body.get("branch"))
                    if "branch" in body
                    else None
                ),
                enabled=(get_bool(body, "enabled") if "enabled" in body else None),
                default_base_repo=None,
                auto_update=(
                    get_bool(body, "auto_update") if "auto_update" in body else None
                ),
            )
            services.library.invalidate_catalog_state(reason="pack_updated")
            return json_success(
                {
                    **payload,
                    "repo": services.ownership.annotate_repo_payload(payload["repo"]),
                },
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def remove_tracked_repo(request: Any) -> Any:
        try:
            owner = request.query.get("owner")
            repo = request.query.get("repo")
            if not isinstance(owner, str) or not owner.strip():
                raise BackendError("'owner' query parameter is required", status=400)
            if not isinstance(repo, str) or not repo.strip():
                raise BackendError("'repo' query parameter is required", status=400)
            payload = services.tracked_repos.remove_repo(owner=owner, repo=repo)
            services.library.invalidate_catalog_state(reason="pack_removed")
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def sync_tracked_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            payload = services.tracked_repos.sync_repo(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
            )
            services.library.invalidate_catalog_state(reason="pack_synced")
            return json_success(
                {
                    **payload,
                    "repo": services.ownership.annotate_repo_payload(payload["repo"]),
                },
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def sync_all_tracked_repos(request: Any) -> Any:
        _ = request
        try:
            payload = services.tracked_repos.sync_all_repos()
            services.library.invalidate_catalog_state(reason="all_packs_synced")
            return json_success(
                services.ownership.annotate_repo_list_payload(payload),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def check_tracked_repo(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            payload = services.tracked_repos.check_repo(
                owner=str(body.get("owner") or ""),
                repo=str(body.get("repo") or ""),
            )
            services.library.invalidate_catalog_state(reason="pack_checked")
            return json_success(
                {
                    **payload,
                    "repo": services.ownership.annotate_repo_payload(payload["repo"]),
                },
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def check_all_tracked_repos(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
            apply_auto_updates = False
            if isinstance(body, Mapping):
                apply_auto_updates = get_bool(body, "apply_auto_updates", False)
            payload = services.tracked_repos.check_all_repos(
                apply_auto_updates=apply_auto_updates
            )
            services.library.invalidate_catalog_state(reason="all_packs_checked")
            return json_success(
                services.ownership.annotate_repo_list_payload(payload),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def list_revisions(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            return json_success(
                services.revisions.list_revisions(cube_id=cube_id),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def load_revision(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            return json_success(
                services.revisions.load_revision(
                    cube_id=body.get("cube_id", ""),
                    revision_ref=body.get("revision_ref", ""),
                    version_pin=normalize_metadata_string(body.get("version_pin")),
                    drop_origin=extract_drop_origin(body.get("origin")) or (0.0, 0.0),
                ),
                status=200,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def preview_cube(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            return json_success(services.library.preview_cube(cube_id), status=200)
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def serve_icon_asset(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            icon_path, media_type = services.library.resolve_cube_icon_asset(cube_id)
            return web.Response(
                body=icon_path.read_bytes(),
                content_type=media_type,
            )
        except BackendError as error:
            return json_error_from_exception(error)
        except OSError:
            _logger.exception("SugarCubes: failed to read cube icon asset")
            return json_error("Failed to read cube icon asset", status=500)

    async def load_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            return json_success(
                services.loader.load_cube(
                    cube_id=body.get("cube_id", ""),
                    version_pin=normalize_metadata_string(body.get("version_pin")),
                    drop_origin=extract_drop_origin(body.get("origin")) or (0.0, 0.0),
                ),
                status=200,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def update_metadata(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            metadata_value = body.get("metadata")
            return json_success(
                services.metadata.update_metadata(
                    cube_id=body.get("cube_id", ""),
                    description_set="description" in body,
                    description=normalize_metadata_string(body.get("description")),
                    version_set="version" in body,
                    version=normalize_metadata_string(body.get("version")),
                    metadata_payload=(
                        metadata_value if isinstance(metadata_value, Mapping) else {}
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def rename_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            if body.get("derive_target_from_name") is True:
                return json_success(
                    services.metadata.rename_cube_from_default_alias(
                        cube_id=body.get("cube_id", ""),
                        target_default_alias=normalize_metadata_string(
                            body.get("default_alias")
                        ),
                    ),
                    status=200,
                )
            return json_success(
                services.metadata.rename_cube(
                    cube_id=body.get("cube_id", ""),
                    target_cube_id=body.get("target_cube_id", ""),
                    target_default_alias=normalize_metadata_string(
                        body.get("default_alias")
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def promote_cube(request: Any) -> Any:
        """Move one personal cube into a claimed writable cube pack."""

        try:
            body = await parse_json_body(request)
            destination = body.get("destination")
            if not isinstance(destination, Mapping):
                raise BackendError("'destination' field is required", status=400)
            return json_success(
                services.promotion.promote(
                    source_cube_id=str(body.get("source_cube_id") or ""),
                    owner=str(destination.get("owner") or ""),
                    repo=str(destination.get("repo") or ""),
                    name=str(body.get("name") or ""),
                    target_model=str(body.get("target_model") or ""),
                    supported_models=body.get("supported_models"),
                    description_set="description" in body,
                    description=normalize_metadata_string(body.get("description")),
                    metadata=(
                        body.get("metadata")
                        if isinstance(body.get("metadata"), Mapping)
                        else {}
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def delete_cube(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        try:
            body = None
            if not isinstance(cube_id, str) or not cube_id.strip():
                body = await parse_optional_json_body(request)
            if isinstance(body, Mapping):
                cube_id = body.get("cube_id")
            normalized_cube_id = normalize_metadata_string(cube_id)
            if not normalized_cube_id:
                return json_error("'cube_id' is required", status=400)
            return json_success(
                services.library.delete_cube(
                    cube_id=normalized_cube_id,
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def import_cube_file(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            source_value = body.get("path") or body.get("source") or body.get("file")
            if not isinstance(source_value, str) or not source_value.strip():
                raise BackendError("'path' field is required", status=400)
            target_cube_id = body.get("cube_id") or body.get("target_cube_id")
            if not isinstance(target_cube_id, str) or not target_cube_id.strip():
                raise BackendError("'cube_id' field is required", status=400)
            return json_success(
                services.library.import_cube_file(
                    source_value=source_value,
                    target_cube_id=target_cube_id,
                    overwrite=get_bool(body, "overwrite", False),
                ),
                status=201,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_many(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            graph_payload = body.get("graph")
            if graph_payload is None:
                raise BackendError("'graph' field is required", status=400)
            workflow_raw = body.get("workflow")
            if workflow_raw is None:
                raise BackendError("'workflow' field is required", status=400)
            actor = normalize_actor(body.get("actor"))
            return json_success(
                services.exporter.save_many(
                    graph=normalize_graph_payload(graph_payload),
                    workflow=normalize_workflow_payload(workflow_raw),
                    workflow_version=coerce_int(
                        body.get("workflow_version"), default=None
                    ),
                    actor=actor or {},
                    cube_entries=parse_save_many_cube_entries(body.get("cubes")),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_implementation(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            graph_payload = body.get("graph")
            if graph_payload is None:
                raise BackendError("'graph' field is required", status=400)
            workflow_raw = body.get("workflow")
            if workflow_raw is None:
                raise BackendError("'workflow' field is required", status=400)
            actor = normalize_actor(body.get("actor"))
            return json_success(
                services.exporter.save_implementation(
                    graph=normalize_graph_payload(graph_payload),
                    workflow=normalize_workflow_payload(workflow_raw),
                    workflow_version=coerce_int(
                        body.get("workflow_version"), default=None
                    ),
                    actor=actor or {},
                    cube_entries=parse_save_many_cube_entries(body.get("cubes")),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_authored_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            values = body.get("values")
            if not isinstance(values, Mapping):
                raise BackendError("'values' field is required", status=400)
            return json_success(
                services.exporter.save_authored_flavor(
                    cube_id=body.get("cube_id", ""),
                    values=values,
                    flavor_id=normalize_metadata_string(body.get("flavor_id")),
                    flavor_name=normalize_metadata_string(body.get("flavor_name")),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def get_local_flavors(request: Any) -> Any:
        try:
            cube_id = request.query.get("cube_id")
            if not isinstance(cube_id, str) or not cube_id.strip():
                raise BackendError("'cube_id' query parameter is required", status=400)
            state = services.local_flavors.read_cube_state(cube_id)
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            values = body.get("values")
            if not isinstance(values, Mapping):
                raise BackendError("'values' field is required", status=400)
            authored_flavors = body.get("authored_flavors")
            state = services.local_flavors.save_local_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                name=str(body.get("name") or ""),
                values=values,
                flavor_id=normalize_metadata_string(body.get("flavor_id")) or None,
                authored_flavors=(
                    authored_flavors if isinstance(authored_flavors, list) else []
                ),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def delete_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            state = services.local_flavors.delete_local_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                flavor_id=str(body.get("flavor_id") or ""),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def select_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            state = services.local_flavors.set_selected_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                flavor_id=str(body.get("flavor_id") or ""),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def migrate_local_flavors(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            states = body.get("states")
            if not isinstance(states, list):
                raise BackendError("'states' field is required", status=400)
            payload = services.local_flavors.migrate_states(states)
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def reconcile_local_flavors(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            authored_flavors = body.get("authored_flavors")
            rename_map = body.get("rename_map")
            payload = services.local_flavors.reconcile_with_authored_flavors(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                authored_flavors=(
                    authored_flavors if isinstance(authored_flavors, list) else []
                ),
                rename_map=rename_map if isinstance(rename_map, Mapping) else None,
            )
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def get_dependency_readiness(request: Any) -> Any:
        _ = request
        try:
            return json_success(services.dependencies.readiness(), status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def repair_dependencies(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
            payload = body if isinstance(body, Mapping) else {}
            approved_node_ids = payload.get("approvedNodeIds")
            if not isinstance(approved_node_ids, list):
                approved_node_ids = []
            baseline_only = get_bool(payload, "baselineOnly", False)
            approve_all = get_bool(payload, "approveAll", False)
            if approve_all:
                approval_policy: DependencyApprovalPolicy = "approve_all"
            elif baseline_only:
                approval_policy = "silent_baseline_only"
            else:
                approval_policy = "approved_node_ids"
            return json_success(
                services.dependencies.repair(
                    approval_policy=approval_policy,
                    approved_node_ids=[
                        normalize_metadata_string(node_id)
                        for node_id in approved_node_ids
                    ],
                    sync_enabled_repos=get_bool(payload, "syncEnabledRepos", False),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def sync_and_check_dependencies(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
            payload = body if isinstance(body, Mapping) else {}
            return json_success(
                services.dependencies.sync_and_check(payload), status=200
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return RouteHandlers(
        list_cubes=list_cubes,
        get_identity_policy=get_identity_policy,
        update_identity_policy=update_identity_policy,
        list_tracked_repos=list_tracked_repos,
        preflight_tracked_repo=preflight_tracked_repo,
        add_tracked_repo=add_tracked_repo,
        create_authoring_repo=create_authoring_repo,
        update_tracked_repo=update_tracked_repo,
        remove_tracked_repo=remove_tracked_repo,
        sync_tracked_repo=sync_tracked_repo,
        sync_all_tracked_repos=sync_all_tracked_repos,
        check_tracked_repo=check_tracked_repo,
        check_all_tracked_repos=check_all_tracked_repos,
        list_revisions=list_revisions,
        load_revision=load_revision,
        preview_cube=preview_cube,
        serve_icon_asset=serve_icon_asset,
        load_cube=load_cube,
        update_metadata=update_metadata,
        rename_cube=rename_cube,
        promote_cube=promote_cube,
        delete_cube=delete_cube,
        import_cube_file=import_cube_file,
        save_many=save_many,
        save_implementation=save_implementation,
        save_authored_flavor=save_authored_flavor,
        get_local_flavors=get_local_flavors,
        save_local_flavor=save_local_flavor,
        delete_local_flavor=delete_local_flavor,
        select_local_flavor=select_local_flavor,
        migrate_local_flavors=migrate_local_flavors,
        reconcile_local_flavors=reconcile_local_flavors,
        get_dependency_readiness=get_dependency_readiness,
        repair_dependencies=repair_dependencies,
        sync_and_check_dependencies=sync_and_check_dependencies,
    )


def register_routes(prompt_server: Any, services: BackendServices) -> RouteHandlers:
    """Register SugarCubes routes on a PromptServer instance."""

    handlers = build_route_handlers(services)
    routes = getattr(prompt_server, "instance", prompt_server).routes
    routes.get("/sugarcubes/list")(handlers.list_cubes)
    routes.get("/sugarcubes/identity_policy")(handlers.get_identity_policy)
    routes.patch("/sugarcubes/identity_policy")(handlers.update_identity_policy)
    routes.get("/sugarcubes/repos")(handlers.list_tracked_repos)
    routes.post("/sugarcubes/repos/preflight")(handlers.preflight_tracked_repo)
    routes.post("/sugarcubes/repos")(handlers.add_tracked_repo)
    routes.post("/sugarcubes/repos/authoring")(handlers.create_authoring_repo)
    routes.patch("/sugarcubes/repos")(handlers.update_tracked_repo)
    routes.delete("/sugarcubes/repos")(handlers.remove_tracked_repo)
    routes.post("/sugarcubes/repos/sync")(handlers.sync_tracked_repo)
    routes.post("/sugarcubes/repos/sync_all")(handlers.sync_all_tracked_repos)
    routes.post("/sugarcubes/packs/check")(handlers.check_tracked_repo)
    routes.post("/sugarcubes/packs/check_all")(handlers.check_all_tracked_repos)
    routes.get("/sugarcubes/revisions")(handlers.list_revisions)
    routes.post("/sugarcubes/load_revision")(handlers.load_revision)
    routes.get("/sugarcubes/preview")(handlers.preview_cube)
    routes.get("/sugarcubes/assets/icon")(handlers.serve_icon_asset)
    routes.post("/sugarcubes/load")(handlers.load_cube)
    routes.post("/sugarcubes/update_metadata")(handlers.update_metadata)
    routes.post("/sugarcubes/rename")(handlers.rename_cube)
    routes.post("/sugarcubes/promote")(handlers.promote_cube)
    routes.delete("/sugarcubes")(handlers.delete_cube)
    routes.post("/sugarcubes/import_file")(handlers.import_cube_file)
    routes.post("/sugarcubes/save_many")(handlers.save_many)
    routes.post("/sugarcubes/save_implementation")(handlers.save_implementation)
    routes.post("/sugarcubes/save_authored_flavor")(handlers.save_authored_flavor)
    routes.get("/sugarcubes/local_flavors")(handlers.get_local_flavors)
    routes.post("/sugarcubes/local_flavors")(handlers.save_local_flavor)
    routes.delete("/sugarcubes/local_flavors")(handlers.delete_local_flavor)
    routes.post("/sugarcubes/local_flavors/select")(handlers.select_local_flavor)
    routes.post("/sugarcubes/local_flavors/migrate")(handlers.migrate_local_flavors)
    routes.post("/sugarcubes/local_flavors/reconcile")(handlers.reconcile_local_flavors)
    routes.get("/sugarcubes/dependencies/readiness")(handlers.get_dependency_readiness)
    routes.post("/sugarcubes/dependencies/repair")(handlers.repair_dependencies)
    routes.post("/sugarcubes/dependencies/sync-and-check")(
        handlers.sync_and_check_dependencies
    )
    return handlers
