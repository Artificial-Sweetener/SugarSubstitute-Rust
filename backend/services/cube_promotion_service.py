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
"""Promote personal cubes into claimed writable cube packs without data loss."""

from __future__ import annotations

import logging
from typing import Any, Callable, Mapping

try:
    from ...cube_model import (
        build_canonical_cube_id,
        normalize_cube_route,
        normalize_supported_models,
        normalize_target_model,
        suggest_canonical_cube_path,
    )
    from ..responses import BackendError
    from .cube_artifact_repository import CubeArtifactRepository
    from .cube_history_service import CubeHistoryService
    from .cube_identity_redirect_service import CubeIdentityRedirectService
    from .cube_library_service import CubeLibraryService, normalize_metadata_string
    from .local_flavor_service import LocalFlavorService
    from .ownership_policy_service import OwnershipPolicyService
except ImportError:
    from cube_model import (
        build_canonical_cube_id,
        normalize_cube_route,
        normalize_supported_models,
        normalize_target_model,
        suggest_canonical_cube_path,
    )
    from backend.responses import BackendError
    from backend.services.cube_artifact_repository import CubeArtifactRepository
    from backend.services.cube_history_service import CubeHistoryService
    from backend.services.cube_identity_redirect_service import (
        CubeIdentityRedirectService,
    )
    from backend.services.cube_library_service import (
        CubeLibraryService,
        normalize_metadata_string,
    )
    from backend.services.local_flavor_service import LocalFlavorService
    from backend.services.ownership_policy_service import OwnershipPolicyService

_logger = logging.getLogger(__name__)
RetargetCubePayload = Callable[..., None]


class CubePromotionService:
    """Own the recoverable application workflow for personal-to-pack promotion."""

    def __init__(
        self,
        *,
        artifacts: CubeArtifactRepository,
        history: CubeHistoryService,
        redirects: CubeIdentityRedirectService,
        local_flavors: LocalFlavorService,
        ownership: OwnershipPolicyService,
        library: CubeLibraryService,
        retarget_cube_payload: RetargetCubePayload,
    ) -> None:
        """Initialize promotion collaborators with one owner per side effect."""

        self.artifacts = artifacts
        self.history = history
        self.redirects = redirects
        self.local_flavors = local_flavors
        self.ownership = ownership
        self.library = library
        self.retarget_cube_payload = retarget_cube_payload

    def promote(
        self,
        *,
        source_cube_id: str,
        owner: str,
        repo: str,
        name: str,
        target_model: str,
        supported_models: Any = None,
        description_set: bool = False,
        description: str = "",
        metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Promote one personal cube and return a recoverable operation result."""

        source_id = normalize_metadata_string(source_cube_id)
        source_parsed = self.artifacts.parse(source_id)
        if (
            source_parsed.source_kind != "local"
            or source_parsed.namespace != "personal"
        ):
            raise BackendError(
                "Only local/personal cubes can be moved to a cube pack", status=400
            )
        normalized_name = self._normalize_name(name)
        normalized_target_model = normalize_target_model(target_model)
        if not normalized_target_model:
            raise BackendError("'target_model' field is required", status=400)
        target_default_alias = normalize_cube_route(
            f"{normalized_target_model}/{normalized_name}"
        )
        target_id = build_canonical_cube_id(
            source_kind="github",
            owner=owner,
            repo=repo,
            path=f"{normalized_target_model}/{suggest_canonical_cube_path(normalized_name)}",
        )
        self.ownership.assert_cube_id_writable(
            target_id, action="move this personal cube into that pack"
        )

        redirect = self.redirects.get(source_id)
        if redirect:
            if redirect.get("target_cube_id") != target_id:
                raise BackendError(
                    "Personal cube was already moved to a different destination",
                    status=409,
                )
            return self._finish_recorded_promotion(source_id, target_id, redirect)

        pending = self.redirects.get_pending(source_id)
        if pending:
            if pending.get("target_cube_id") != target_id:
                raise BackendError(
                    "Personal cube already has a pending move to a different destination",
                    status=409,
                )
            resumed = self._resume_pending_promotion(source_id, target_id, pending)
            if resumed is not None:
                return resumed

        source_context, source_payload = self.artifacts.read(source_id)
        target_context = self.artifacts.assert_available(target_id)
        target_payload = self._build_target_payload(
            source_payload=source_payload,
            source_cube_id=source_id,
            target_cube_id=target_id,
            target_default_alias=target_default_alias,
            target_model=normalized_target_model,
            supported_models=supported_models,
            description_set=description_set,
            description=description,
            metadata=metadata or {},
        )
        version = normalize_metadata_string(target_payload.get("version"))

        self.redirects.begin_promotion(
            source_cube_id=source_id,
            target_cube_id=target_id,
            source_relative_path=source_context.repo_relative_path,
            version=version,
        )
        self.artifacts.write(target_context, target_payload)
        try:
            target_commit = self.history.commit_context(
                target_context,
                message=f"promote {normalized_name}.cube at v{version or 'unversioned'}",
            )
        except BackendError:
            self.artifacts.delete(target_context)
            self._unstage_context(target_context)
            self.redirects.clear_pending(source_id)
            raise

        redirect_record = self.redirects.record_promotion(
            source_cube_id=source_id,
            target_cube_id=target_id,
            source_relative_path=source_context.repo_relative_path,
            source_commit_sha="",
            target_commit_sha=target_commit.commit_sha,
            version=version,
        )
        flavor_result: dict[str, Any] = {"moved": False}
        try:
            flavor_result = self.local_flavors.move_cube_state(source_id, target_id)
            source_commit = self._retire_source(
                source_context, source_payload, normalized_name
            )
        except BackendError as exc:
            _logger.warning(
                "SugarCubes: promotion copied '%s' but personal cleanup is pending",
                source_id,
                exc_info=exc,
            )
            self.library.invalidate_catalog_state(
                reason="cube_promotion_cleanup_pending",
                affected_cube_ids=[source_id, target_id],
            )
            return {
                "status": "cleanup_pending",
                "previous_cube_id": source_id,
                "cube": self.library.summarize_cube(target_context.cube_path),
                "version": version,
                "redirect": redirect_record,
                "local_flavors": flavor_result,
                "commits": {
                    "target": self._commit_payload(target_commit),
                    "source": None,
                },
                "cleanup_error": exc.message,
            }

        redirect_record = self.redirects.record_promotion(
            source_cube_id=source_id,
            target_cube_id=target_id,
            source_relative_path=source_context.repo_relative_path,
            source_commit_sha=source_commit.commit_sha,
            target_commit_sha=target_commit.commit_sha,
            version=version,
        )
        self.library.invalidate_catalog_state(
            reason="cube_promoted", affected_cube_ids=[source_id, target_id]
        )
        return {
            "status": "complete",
            "previous_cube_id": source_id,
            "cube": self.library.summarize_cube(target_context.cube_path),
            "version": version,
            "redirect": redirect_record,
            "local_flavors": flavor_result,
            "commits": {
                "target": self._commit_payload(target_commit),
                "source": self._commit_payload(source_commit),
            },
        }

    def _resume_pending_promotion(
        self,
        source_cube_id: str,
        target_cube_id: str,
        pending: Mapping[str, Any],
    ) -> dict[str, Any] | None:
        """Resume after a target commit succeeded but redirect persistence was interrupted."""

        target_context = self.artifacts.context(target_cube_id)
        if not target_context.cube_path.is_file():
            self.redirects.clear_pending(source_cube_id)
            return None
        if self.history.tracked_repo_service.has_file_changes(
            repo_root=target_context.repo_root,
            repo_relative_path=target_context.repo_relative_path,
        ):
            self.artifacts.delete(target_context)
            self._unstage_context(target_context)
            self.redirects.clear_pending(source_cube_id)
            return None
        target_commit_sha = self.history.latest_path_commit(target_context)
        if not target_commit_sha:
            self.artifacts.delete(target_context)
            self.redirects.clear_pending(source_cube_id)
            return None
        redirect = self.redirects.record_promotion(
            source_cube_id=source_cube_id,
            target_cube_id=target_cube_id,
            source_relative_path=normalize_metadata_string(
                pending.get("source_relative_path")
            ),
            source_commit_sha="",
            target_commit_sha=target_commit_sha,
            version=normalize_metadata_string(pending.get("version")),
        )
        return self._finish_recorded_promotion(source_cube_id, target_cube_id, redirect)

    def _finish_recorded_promotion(
        self, source_cube_id: str, target_cube_id: str, redirect: Mapping[str, Any]
    ) -> dict[str, Any]:
        """Finish pending personal cleanup or return an idempotent completed result."""

        target_context, target_payload = self.artifacts.read(target_cube_id)
        version = normalize_metadata_string(target_payload.get("version"))
        try:
            source_context, source_payload = self.artifacts.read(source_cube_id)
        except BackendError as exc:
            if exc.status != 404:
                raise
            return {
                "status": "complete",
                "previous_cube_id": source_cube_id,
                "cube": self.library.summarize_cube(target_context.cube_path),
                "version": version,
                "redirect": dict(redirect),
                "local_flavors": {"moved": False},
                "commits": {"target": None, "source": None},
            }
        flavor_result = self.local_flavors.move_cube_state(
            source_cube_id, target_cube_id
        )
        source_commit = self._retire_source(
            source_context,
            source_payload,
            target_context.cube_path.stem,
        )
        updated_redirect = self.redirects.record_promotion(
            source_cube_id=source_cube_id,
            target_cube_id=target_cube_id,
            source_relative_path=source_context.repo_relative_path,
            source_commit_sha=source_commit.commit_sha,
            target_commit_sha=normalize_metadata_string(
                redirect.get("target_commit_sha")
            ),
            version=version,
        )
        self.library.invalidate_catalog_state(
            reason="cube_promotion_cleanup_completed",
            affected_cube_ids=[source_cube_id, target_cube_id],
        )
        return {
            "status": "complete",
            "previous_cube_id": source_cube_id,
            "cube": self.library.summarize_cube(target_context.cube_path),
            "version": version,
            "redirect": updated_redirect,
            "local_flavors": flavor_result,
            "commits": {"target": None, "source": self._commit_payload(source_commit)},
        }

    def _build_target_payload(
        self,
        *,
        source_payload: Mapping[str, Any],
        source_cube_id: str,
        target_cube_id: str,
        target_default_alias: str,
        target_model: str,
        supported_models: Any,
        description_set: bool,
        description: str,
        metadata: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Retarget one payload and apply explicit pack-facing metadata."""

        payload = _json_clone(source_payload)
        previous_default_alias = normalize_metadata_string(
            payload.get("metadata", {}).get("default_alias")
            if isinstance(payload.get("metadata"), Mapping)
            else ""
        )
        self.retarget_cube_payload(
            payload,
            previous_cube_id=source_cube_id,
            target_cube_id=target_cube_id,
            previous_default_alias=previous_default_alias,
            target_default_alias=target_default_alias,
        )
        payload["cube_id"] = target_cube_id
        if description_set:
            payload["description"] = normalize_metadata_string(description)
        next_metadata = (
            dict(payload.get("metadata"))
            if isinstance(payload.get("metadata"), Mapping)
            else {}
        )
        next_metadata["default_alias"] = target_default_alias
        next_metadata["target_model"] = target_model
        next_metadata["supported_models"] = normalize_supported_models(
            supported_models, target_model=target_model
        )
        for key in ("author_url", "icon", "tags"):
            if key in metadata:
                next_metadata[key] = _json_clone(metadata[key])
        next_metadata.pop("author", None)
        payload["metadata"] = next_metadata
        return payload

    def _retire_source(
        self, source_context: Any, source_payload: Mapping[str, Any], name: str
    ) -> Any:
        """Delete and commit the personal source, restoring it if the commit fails."""

        self.artifacts.delete(source_context)
        try:
            return self.history.commit_context(
                source_context, message=f"promote {name}.cube to cube pack"
            )
        except BackendError:
            self.artifacts.restore(source_context, source_payload)
            try:
                self.history.tracked_repo_service.git_runner(
                    ["reset", "HEAD", "--", source_context.repo_relative_path],
                    cwd=source_context.repo_root,
                )
            except RuntimeError:
                _logger.warning(
                    "SugarCubes: failed to unstage restored personal cube",
                    exc_info=True,
                )
            raise

    def _unstage_context(self, context: Any) -> None:
        """Best-effort unstage one artifact after a rolled-back target write."""

        try:
            self.history.tracked_repo_service.git_runner(
                ["reset", "HEAD", "--", context.repo_relative_path],
                cwd=context.repo_root,
            )
        except RuntimeError:
            _logger.warning(
                "SugarCubes: failed to unstage rolled-back promotion target",
                exc_info=True,
            )

    def _normalize_name(self, value: str) -> str:
        """Return one path-safe display name without accepting route input."""

        normalized = normalize_cube_route(value)
        if not normalized or "/" in normalized:
            raise BackendError(
                "Cube name must be one non-empty path segment", status=400
            )
        suggest_canonical_cube_path(normalized)
        return normalized

    def _commit_payload(self, commit: Any) -> dict[str, str]:
        """Project one commit result into the promotion response."""

        return {
            "commit_sha": commit.commit_sha,
            "commit_short_sha": commit.commit_short_sha,
            "commit_message": commit.commit_message,
        }


def _json_clone(value: Any) -> Any:
    """Clone one JSON-compatible value without retaining mutable aliases."""

    import json

    return json.loads(json.dumps(value))
