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

"""Adapt identity and tracked-repository workflows to HTTP responses."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .services.cube_metadata import normalize_metadata_string
from .validation import get_bool, parse_json_body, parse_optional_json_body


@dataclass(frozen=True)
class RepositoryRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

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


def build_repository_route_handlers(
    services: BackendServices,
) -> RepositoryRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

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

    return RepositoryRouteHandlers(
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
    )


__all__ = ["RepositoryRouteHandlers", "build_repository_route_handlers"]
