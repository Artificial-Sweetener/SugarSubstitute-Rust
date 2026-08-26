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
"""Compose and register SugarCubes backend HTTP route families."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .catalog_routes import build_catalog_route_handlers
from .composition import BackendServices
from .cube_routes import build_cube_route_handlers
from .dependency_routes import build_dependency_route_handlers
from .export_routes import build_export_route_handlers
from .execution_routes import build_execution_route_handlers
from .flavor_routes import build_flavor_route_handlers
from .implementation_save_routes import build_implementation_save_route_handlers
from .legacy_workflow_routes import build_legacy_workflow_route_handlers
from .repository_routes import build_repository_route_handlers
from .route_types import RouteHandler
from .sugarscript_routes import build_sugarscript_route_handlers
from .workflow_library_routes import build_workflow_library_route_handlers


@dataclass(frozen=True)
class RouteHandlers:
    """Concrete route callables used for registration and tests."""

    get_status: RouteHandler
    list_cubes: RouteHandler
    list_picker_catalog: RouteHandler
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
    preview_implementation: RouteHandler
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
    classify_workflow: RouteHandler
    save_workflow_cube_to_stable: RouteHandler
    fork_workflow_cube: RouteHandler
    sync_workflow_cube_source: RouteHandler
    compile_sugarscript: RouteHandler
    compile_legacy_workflow: RouteHandler
    queue_execution: RouteHandler


def build_route_handlers(services: BackendServices) -> RouteHandlers:
    """Compose endpoint-family handlers behind the stable route contract."""

    catalog = build_catalog_route_handlers(services)
    repositories = build_repository_route_handlers(services)
    cubes = build_cube_route_handlers(services)
    exports = build_export_route_handlers(services)
    implementation_save = build_implementation_save_route_handlers(services)
    flavors = build_flavor_route_handlers(services)
    dependencies = build_dependency_route_handlers(services)
    workflow_library = build_workflow_library_route_handlers(services)
    sugarscript = build_sugarscript_route_handlers(services)
    legacy_workflow = build_legacy_workflow_route_handlers(services)
    execution = build_execution_route_handlers(services)
    return RouteHandlers(
        get_status=catalog.get_status,
        list_cubes=catalog.list_cubes,
        list_picker_catalog=catalog.list_picker_catalog,
        get_identity_policy=repositories.get_identity_policy,
        update_identity_policy=repositories.update_identity_policy,
        list_tracked_repos=repositories.list_tracked_repos,
        preflight_tracked_repo=repositories.preflight_tracked_repo,
        add_tracked_repo=repositories.add_tracked_repo,
        create_authoring_repo=repositories.create_authoring_repo,
        update_tracked_repo=repositories.update_tracked_repo,
        remove_tracked_repo=repositories.remove_tracked_repo,
        sync_tracked_repo=repositories.sync_tracked_repo,
        sync_all_tracked_repos=repositories.sync_all_tracked_repos,
        check_tracked_repo=repositories.check_tracked_repo,
        check_all_tracked_repos=repositories.check_all_tracked_repos,
        list_revisions=cubes.list_revisions,
        load_revision=cubes.load_revision,
        preview_cube=cubes.preview_cube,
        serve_icon_asset=cubes.serve_icon_asset,
        load_cube=cubes.load_cube,
        update_metadata=cubes.update_metadata,
        rename_cube=cubes.rename_cube,
        promote_cube=cubes.promote_cube,
        delete_cube=cubes.delete_cube,
        import_cube_file=cubes.import_cube_file,
        save_many=exports.save_many,
        preview_implementation=implementation_save.preview,
        save_implementation=implementation_save.save,
        save_authored_flavor=exports.save_authored_flavor,
        get_local_flavors=flavors.get_local_flavors,
        save_local_flavor=flavors.save_local_flavor,
        delete_local_flavor=flavors.delete_local_flavor,
        select_local_flavor=flavors.select_local_flavor,
        migrate_local_flavors=flavors.migrate_local_flavors,
        reconcile_local_flavors=flavors.reconcile_local_flavors,
        get_dependency_readiness=dependencies.get_dependency_readiness,
        repair_dependencies=dependencies.repair_dependencies,
        sync_and_check_dependencies=dependencies.sync_and_check_dependencies,
        classify_workflow=workflow_library.classify_workflow,
        save_workflow_cube_to_stable=workflow_library.save_workflow_cube_to_stable,
        fork_workflow_cube=workflow_library.fork_workflow_cube,
        sync_workflow_cube_source=workflow_library.sync_workflow_cube_source,
        compile_sugarscript=sugarscript.compile_sugarscript,
        compile_legacy_workflow=legacy_workflow.compile_legacy_workflow,
        queue_execution=execution.queue_execution,
    )


def register_routes(prompt_server: Any, services: BackendServices) -> RouteHandlers:
    """Register SugarCubes routes on a PromptServer instance."""

    handlers = build_route_handlers(services)
    routes = getattr(prompt_server, "instance", prompt_server).routes
    routes.get("/sugarcubes/status")(handlers.get_status)
    routes.get("/sugarcubes/list")(handlers.list_cubes)
    routes.get("/sugarcubes/picker_catalog")(handlers.list_picker_catalog)
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
    routes.post("/sugarcubes/save_implementation/preview")(
        handlers.preview_implementation
    )
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
    routes.post("/sugarcubes/v2/cubes/classify-workflow")(handlers.classify_workflow)
    routes.post("/sugarcubes/v2/cubes/stable")(handlers.save_workflow_cube_to_stable)
    routes.post("/sugarcubes/v2/cubes/forks")(handlers.fork_workflow_cube)
    routes.post("/sugarcubes/v2/cubes/sync-source")(handlers.sync_workflow_cube_source)
    routes.post("/sugarcubes/v2/sugarscript/compile")(handlers.compile_sugarscript)
    routes.post("/sugarcubes/v2/workflows/compile-legacy")(
        handlers.compile_legacy_workflow
    )
    routes.post("/sugarcubes/v2/executions/queue")(handlers.queue_execution)
    return handlers
