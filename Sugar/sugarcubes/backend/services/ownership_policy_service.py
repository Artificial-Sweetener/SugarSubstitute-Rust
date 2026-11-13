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
"""Derive repo and cube writability from the local identity policy."""

from __future__ import annotations

from typing import Any, Mapping, Optional

from ...cube_model import CubeIdentityError, parse_canonical_cube_id
from ..responses import BackendError
from .identity_policy_service import IdentityPolicyService
from .tracked_repo_service import TrackedRepoService

_SYSTEM_OWNER = "artificial-sweetener"
_NO_OWNER_REASON = "Tracked GitHub repos are read-only until you claim one GitHub owner in SugarCubes settings."
_MISMATCH_REASON = "This tracked repo does not match the claimed GitHub owner."
_SYSTEM_GATE_REASON = "Claiming artificial-sweetener is disabled until SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM is enabled in .env or the process environment."
_TRACK_REPO_REASON = "Track this repo locally before saving into it."


class OwnershipPolicyService:
    """Own repo and cube writability decisions for SugarCubes."""

    def __init__(
        self,
        *,
        tracked_repo_service: TrackedRepoService,
        identity_policy_service: IdentityPolicyService,
    ) -> None:
        """Initialize one ownership policy service."""

        self.tracked_repo_service = tracked_repo_service
        self.identity_policy_service = identity_policy_service

    def list_identity_policy(self) -> dict[str, Any]:
        """Return the serialized identity policy payload."""

        return self.identity_policy_service.serialize_policy()

    def update_identity_policy(
        self,
        *,
        claimed_github_owner: Optional[str] = None,
    ) -> dict[str, Any]:
        """Persist an identity policy update and return the response payload."""

        return self.identity_policy_service.set_policy(
            claimed_github_owner=claimed_github_owner,
        )

    def annotate_repo_payload(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Attach ownership policy metadata to one tracked repo payload."""

        repo = dict(payload)
        owner = str(repo.get("owner") or "")
        name = str(repo.get("repo") or "")
        policy = self.describe_tracked_repo(owner=owner, repo=name)
        repo.update(policy)
        return repo

    def annotate_repo_list_payload(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Attach ownership policy metadata to one tracked repo list payload."""

        repos = payload.get("repos")
        if not isinstance(repos, list):
            return dict(payload)
        return {
            **dict(payload),
            "repos": [
                self.annotate_repo_payload(entry)
                for entry in repos
                if isinstance(entry, Mapping)
            ],
            "identity_policy": self.list_identity_policy(),
        }

    def annotate_cube_payload(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Attach ownership policy metadata to one cube summary payload."""

        cube = dict(payload)
        source = cube.get("source")
        if not isinstance(source, Mapping):
            return cube
        policy = self.describe_source(
            source_kind=str(source.get("type") or ""),
            owner=str(source.get("owner") or ""),
            repo=str(source.get("repo") or ""),
            namespace=str(source.get("namespace") or ""),
            require_tracked_repo=False,
        )
        cube.update(
            {
                "ownership_mode": policy["ownership_mode"],
                "is_system_pack": policy["is_system_pack"],
                "is_writable": policy["is_writable"],
                "write_target_kind": policy["write_target_kind"],
                "write_block_reason": policy["write_block_reason"],
            }
        )
        return cube

    def describe_tracked_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Return policy metadata for one tracked GitHub repo."""

        return self.describe_source(
            source_kind="github",
            owner=owner,
            repo=repo,
            namespace="",
            require_tracked_repo=True,
        )

    def assert_authoring_repo_allowed(
        self, *, owner: str, repo: str
    ) -> tuple[str, str]:
        """Reject authoring-pack creation when the requested owner is not writable."""

        try:
            parsed = parse_canonical_cube_id(f"{owner}/{repo}/placeholder.cube")
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        policy = self.describe_source(
            source_kind="github",
            owner=parsed.owner,
            repo=parsed.repo,
            namespace="",
            require_tracked_repo=False,
        )
        if policy["is_writable"]:
            return parsed.owner, parsed.repo
        raise BackendError(
            f"Cannot create authoring pack: {policy['write_block_reason']}",
            status=403,
            details={
                "owner": parsed.owner,
                "repo": parsed.repo,
                "write_block_reason": policy["write_block_reason"],
            },
        )

    def describe_source(
        self,
        *,
        source_kind: str,
        owner: str,
        repo: str,
        namespace: str,
        require_tracked_repo: bool,
    ) -> dict[str, Any]:
        """Return ownership metadata for one source-qualified cube location."""

        if source_kind == "local":
            return {
                "ownership_mode": "mine",
                "is_system_pack": False,
                "is_writable": True,
                "write_target_kind": "local",
                "write_block_reason": "",
                "namespace": namespace,
            }

        policy = self.identity_policy_service.get_policy()
        normalized_owner = owner.strip()
        normalized_repo = repo.strip()
        is_system_pack = normalized_owner.lower() == _SYSTEM_OWNER
        claimed_owner = policy.claimed_github_owner.strip()
        owner_matches = (
            bool(claimed_owner) and claimed_owner.lower() == normalized_owner.lower()
        )
        tracked_exists = True
        if require_tracked_repo:
            try:
                self.tracked_repo_service.get_repo(normalized_owner, normalized_repo)
            except BackendError as exc:
                if exc.status == 404:
                    tracked_exists = False
                else:
                    raise

        write_block_reason = ""
        is_writable = False
        if (
            is_system_pack
            and claimed_owner.lower() == _SYSTEM_OWNER
            and not policy.allow_system_owner_claim
        ):
            write_block_reason = _SYSTEM_GATE_REASON
        elif not claimed_owner:
            write_block_reason = _NO_OWNER_REASON
        elif not owner_matches:
            write_block_reason = _MISMATCH_REASON
        elif require_tracked_repo and not tracked_exists:
            write_block_reason = _TRACK_REPO_REASON
        else:
            is_writable = True

        return {
            "ownership_mode": "mine" if is_writable else "external",
            "is_system_pack": is_system_pack,
            "is_writable": is_writable,
            "write_target_kind": "tracked_owned_repo" if is_writable else "read_only",
            "write_block_reason": write_block_reason,
        }

    def assert_cube_id_writable(self, cube_id: str, *, action: str) -> None:
        """Reject writes to non-writable cube ids with actionable messages."""

        try:
            parsed = parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        if parsed.source_kind == "local":
            return
        policy = self.describe_source(
            source_kind="github",
            owner=parsed.owner,
            repo=parsed.repo,
            namespace="",
            require_tracked_repo=True,
        )
        if policy["is_writable"]:
            return
        status = 409 if policy["write_block_reason"] == _TRACK_REPO_REASON else 403
        raise BackendError(
            f"Cannot {action}: {policy['write_block_reason']}",
            status=status,
            details={
                "cube_id": cube_id,
                "owner": parsed.owner,
                "repo": parsed.repo,
                "write_block_reason": policy["write_block_reason"],
            },
        )
