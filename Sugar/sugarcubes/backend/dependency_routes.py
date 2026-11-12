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

"""Adapt dependency readiness and repair workflows to HTTP responses."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .services.dependency_approval_policy import DependencyApprovalPolicy
from .services.cube_metadata import normalize_metadata_string
from .validation import get_bool, parse_optional_json_body


@dataclass(frozen=True)
class DependencyRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

    get_dependency_readiness: RouteHandler
    repair_dependencies: RouteHandler
    sync_and_check_dependencies: RouteHandler


def build_dependency_route_handlers(
    services: BackendServices,
) -> DependencyRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

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

    return DependencyRouteHandlers(
        get_dependency_readiness=get_dependency_readiness,
        repair_dependencies=repair_dependencies,
        sync_and_check_dependencies=sync_and_check_dependencies,
    )


__all__ = ["DependencyRouteHandlers", "build_dependency_route_handlers"]
