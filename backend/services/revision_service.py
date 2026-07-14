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
"""Git-backed cube revision services for SugarCubes."""

from __future__ import annotations

import json
import logging
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any, Callable, Mapping, Sequence

try:
    from ...importer import CubeImportError
    from ...instrumentation import log_diagnostic
    from ..responses import BackendError
    from .cube_icon_service import attach_icon_url, normalize_existing_icon_metadata
    from .cube_identity_redirect_service import CubeIdentityRedirectService
    from .cube_git_context import CubeGitContext, resolve_cube_git_context
    from .cube_library_service import (
        CubeLibraryService,
        build_cube_identity_fields,
        format_timestamp,
        normalize_metadata_string,
        read_cube_payload,
    )
    from .tracked_repo_service import TrackedRepoService
except ImportError:
    from importer import CubeImportError
    from instrumentation import log_diagnostic
    from backend.responses import BackendError
    from backend.services.cube_icon_service import (
        attach_icon_url,
        normalize_existing_icon_metadata,
    )
    from backend.services.cube_identity_redirect_service import (
        CubeIdentityRedirectService,
    )
    from backend.services.cube_git_context import (
        CubeGitContext,
        resolve_cube_git_context,
    )
    from backend.services.cube_library_service import (
        CubeLibraryService,
        build_cube_identity_fields,
        format_timestamp,
        normalize_metadata_string,
        read_cube_payload,
    )
    from backend.services.tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_CURRENT_REVISION_REF = "WORKTREE"
CUBE_REVISION_TRACE_MARKER = "SugarCubes cube revision diagnostic"


class CubeRevisionService:
    """Own git-backed revision listing and historical cube loading."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        tracked_repo_service: TrackedRepoService,
        *,
        load_cube_artifact: Callable[[Any], Any],
        prepare_cube_import: Callable[..., Any],
        redirect_service: CubeIdentityRedirectService,
    ) -> None:
        """Initialize the revision service."""

        self.library_service = library_service
        self.tracked_repo_service = tracked_repo_service
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.redirect_service = redirect_service

    def list_revisions(self, *, cube_id: str) -> dict[str, Any]:
        """Return available git-backed revisions for one canonical cube id."""

        _log_cube_library_diagnostic("sugarcubes_revision_list_start", cube_id=cube_id)
        requested_cube_id = normalize_metadata_string(cube_id)
        context = self._resolve_git_context(requested_cube_id)
        revisions = [self._build_current_revision(context)]
        revisions.extend(self._list_committed_revisions(context))
        revisions = self._drop_redundant_current_head_revision(revisions, context)
        redirect = self.redirect_service.get(requested_cube_id)
        if redirect and requested_cube_id != context.cube_id:
            source_context = resolve_cube_git_context(
                self.tracked_repo_service, requested_cube_id
            )
            personal_history = [
                {
                    **entry,
                    "label": f"Personal {entry['label']}",
                    "history_origin": "personal",
                }
                for entry in self._list_committed_revisions(source_context)
                if entry.get("version")
            ]
            revisions.extend(personal_history)
        _log_cube_library_diagnostic(
            "sugarcubes_revision_list_return",
            cube_id=context.cube_id,
            revision_count=len(revisions),
            revision_refs=[
                normalize_metadata_string(revision.get("revision_ref"))
                for revision in revisions
            ],
        )
        result = {
            "cube_id": context.cube_id,
            "revisions": revisions,
            "count": len(revisions),
            "duplicate_version_omissions": [],
        }
        if requested_cube_id != context.cube_id:
            result["identity_redirect"] = {
                "requested_cube_id": requested_cube_id,
                "resolved_cube_id": context.cube_id,
            }
        return result

    def load_revision(
        self,
        *,
        cube_id: str,
        revision_ref: str,
        version_pin: str,
        drop_origin: Sequence[float],
    ) -> dict[str, Any]:
        """Load one current or historical cube revision for frontend import."""

        _log_cube_library_diagnostic(
            "sugarcubes_revision_load_start",
            cube_id=cube_id,
            revision_ref=revision_ref,
            version_pin=version_pin,
        )
        requested_cube_id = normalize_metadata_string(cube_id)
        context = self._resolve_git_context(requested_cube_id)
        normalized_revision_ref = (
            normalize_metadata_string(revision_ref) or _CURRENT_REVISION_REF
        )
        if normalized_revision_ref == _CURRENT_REVISION_REF:
            return self._load_current_revision(
                context,
                version_pin=version_pin,
                drop_origin=drop_origin,
            )
        revision_context, payload_text = self._load_historical_payload(
            requested_cube_id=requested_cube_id,
            current_context=context,
            revision_ref=normalized_revision_ref,
        )
        try:
            payload = json.loads(payload_text)
        except json.JSONDecodeError as exc:
            raise BackendError(
                "Revision payload is not valid JSON", status=500
            ) from exc
        with TemporaryDirectory(prefix="sugarcubes-revision-") as temp_dir:
            temp_path = Path(temp_dir) / Path(revision_context.repo_relative_path).name
            temp_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            return self._load_path(
                temp_path,
                revision_context,
                revision_ref=normalized_revision_ref,
                current=False,
                version_pin=version_pin,
                drop_origin=drop_origin,
            )

    def _load_current_revision(
        self,
        context: CubeGitContext,
        *,
        version_pin: str,
        drop_origin: Sequence[float],
    ) -> dict[str, Any]:
        """Load the current working-tree cube revision."""

        return self._load_path(
            context.cube_path,
            context,
            revision_ref=_CURRENT_REVISION_REF,
            current=True,
            version_pin=version_pin,
            drop_origin=drop_origin,
        )

    def _load_path(
        self,
        cube_path: Path,
        context: CubeGitContext,
        *,
        revision_ref: str,
        current: bool,
        version_pin: str,
        drop_origin: Sequence[float],
    ) -> dict[str, Any]:
        """Load one cube file path and shape the importer response."""

        try:
            loaded_cube = self.load_cube_artifact(cube_path)
            if version_pin and loaded_cube.version != version_pin:
                _log_cube_library_diagnostic(
                    "sugarcubes_revision_version_pin_mismatch",
                    cube_id=context.cube_id,
                    revision_ref=revision_ref,
                    expected_version=version_pin,
                    actual_version=loaded_cube.version,
                )
                raise BackendError(
                    "Cube version mismatch",
                    status=409,
                    details={"expected": version_pin, "actual": loaded_cube.version},
                )
            prepared = self.prepare_cube_import(loaded_cube, drop_origin=drop_origin)
        except CubeImportError:
            raise
        except BackendError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception(
                "SugarCubes: failed to load revision '%s' for cube '%s'",
                revision_ref,
                context.cube_id,
            )
            raise BackendError("Load failed", status=500) from exc

        cube_payload = dict(prepared.cube)
        cube_payload.setdefault("name", context.cube_path.stem)
        metadata = (
            cube_payload.get("metadata")
            if isinstance(cube_payload.get("metadata"), dict)
            else {}
        )
        icon = attach_icon_url(
            normalize_existing_icon_metadata(metadata.get("icon")),
            normalize_metadata_string(cube_payload.get("cube_id")),
        )
        if icon:
            cube_payload["icon"] = icon
        cube_payload.update(
            build_cube_identity_fields(
                cube_id=normalize_metadata_string(cube_payload.get("cube_id"))
                or context.cube_id,
                default_alias=normalize_metadata_string(metadata.get("default_alias"))
                or normalize_metadata_string(cube_payload.get("name"))
                or context.cube_path.stem,
                metadata=metadata,
            )
        )
        _log_cube_library_diagnostic(
            "sugarcubes_revision_load_return",
            cube_id=context.cube_id,
            revision_ref=revision_ref,
            current=current,
            loaded_cube_id=normalize_metadata_string(cube_payload.get("cube_id")),
            loaded_version=normalize_metadata_string(cube_payload.get("version")),
        )
        return {
            "cube": cube_payload,
            "nodes": prepared.nodes,
            "markers": prepared.markers,
            "connections": prepared.connections,
            "layout": prepared.layout,
            "warnings": prepared.warnings,
            "subgraphs": prepared.subgraphs,
            "source": {
                "path": str(context.cube_path),
                "name": context.cube_path.stem,
                "type": context.source_kind,
                "owner": context.owner,
                "repo": context.repo,
                "repo_ref": (
                    f"{context.owner}/{context.repo}"
                    if context.owner and context.repo
                    else ""
                ),
                "namespace": context.namespace,
                "relative_path": context.repo_relative_path,
            },
            "revision": {
                "revision_ref": revision_ref,
                "current": current,
            },
        }

    def _resolve_git_context(self, cube_id: str) -> CubeGitContext:
        """Resolve the owning git repo and repo-relative cube path."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        resolved_cube_id = self.redirect_service.resolve(normalized_cube_id)
        context = resolve_cube_git_context(self.tracked_repo_service, resolved_cube_id)
        if not context.cube_path.exists() or not context.cube_path.is_file():
            raise BackendError(f"Cube '{resolved_cube_id}' not found", status=404)
        return context

    def _load_historical_payload(
        self,
        *,
        requested_cube_id: str,
        current_context: CubeGitContext,
        revision_ref: str,
    ) -> tuple[CubeGitContext, str]:
        """Load a revision from current history or redirected personal history."""

        try:
            return current_context, self._git_show(current_context, revision_ref)
        except BackendError as current_error:
            redirect = self.redirect_service.get(requested_cube_id)
            if not redirect or requested_cube_id == current_context.cube_id:
                raise current_error
            source_context = resolve_cube_git_context(
                self.tracked_repo_service, requested_cube_id
            )
            try:
                return source_context, self._git_show(source_context, revision_ref)
            except BackendError:
                raise current_error

    def _build_current_revision(self, context: CubeGitContext) -> dict[str, Any]:
        """Build the synthetic current working-tree revision entry."""

        payload, error = read_cube_payload(context.cube_path)
        version = ""
        if payload and not error:
            version_value = payload.get("version")
            version = version_value.strip() if isinstance(version_value, str) else ""
        stat_info = context.cube_path.stat()
        return {
            "revision_ref": _CURRENT_REVISION_REF,
            "label": "Current",
            "current": True,
            "committed": False,
            "timestamp": format_timestamp(stat_info.st_mtime),
            "version": version,
            "source_type": context.source_kind,
            "commit_sha": "",
            "short_sha": "",
            "subject": "Current working tree",
        }

    def _list_committed_revisions(
        self,
        context: CubeGitContext,
    ) -> list[dict[str, Any]]:
        """Return committed git revisions for one cube file."""

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
            no_history = (
                "does not have any commits yet" in message
                or "unknown revision" in message
            )
            if no_history:
                return []
            raise BackendError("Failed to list cube revisions", status=500) from exc

        entries: list[dict[str, Any]] = []
        for line in (result.stdout or "").splitlines():
            parts = line.split("\x1f")
            if len(parts) != 3:
                continue
            revision_ref, timestamp, subject = parts
            version = self._read_revision_version(context, revision_ref)
            entries.append(
                {
                    "revision_ref": revision_ref,
                    "label": revision_ref[:7],
                    "current": False,
                    "committed": True,
                    "timestamp": timestamp,
                    "version": version,
                    "source_type": context.source_kind,
                    "commit_sha": revision_ref,
                    "short_sha": revision_ref[:7],
                    "subject": subject,
                }
            )
        return entries

    def _read_revision_version(
        self,
        context: CubeGitContext,
        revision_ref: str,
    ) -> str:
        """Read the payload version for one committed cube revision."""

        try:
            payload_text = self._git_show(context, revision_ref)
        except BackendError:
            return ""
        try:
            payload = json.loads(payload_text)
        except json.JSONDecodeError:
            return ""
        version_value = payload.get("version")
        return version_value.strip() if isinstance(version_value, str) else ""

    def _drop_redundant_current_head_revision(
        self,
        revisions: Sequence[dict[str, Any]],
        context: CubeGitContext,
    ) -> list[dict[str, Any]]:
        """Remove the committed HEAD mirror when it has the same version as WORKTREE."""

        entries = list(revisions)
        if len(entries) < 2:
            return entries
        current = entries[0]
        head = entries[1]
        current_version = normalize_metadata_string(current.get("version"))
        head_version = normalize_metadata_string(head.get("version"))
        if (
            normalize_metadata_string(current.get("revision_ref"))
            == _CURRENT_REVISION_REF
            and current_version
            and current_version == head_version
            and bool(head.get("committed"))
            and self._current_matches_revision(context, head)
        ):
            return [current, *entries[2:]]
        return entries

    def _current_matches_revision(
        self,
        context: CubeGitContext,
        revision: Mapping[str, Any],
    ) -> bool:
        """Return whether the current cube file exactly matches one git revision."""

        revision_ref = normalize_metadata_string(revision.get("revision_ref"))
        if not revision_ref:
            return False
        try:
            current_text = context.cube_path.read_text(encoding="utf-8")
            revision_text = self._git_show(context, revision_ref)
        except (OSError, BackendError):
            return False
        return current_text.strip() == revision_text.strip()

    def _git_show(self, context: CubeGitContext, revision_ref: str) -> str:
        """Return the historical file contents for one git revision."""

        try:
            result = self.tracked_repo_service.git_runner(
                ["show", f"{revision_ref}:{context.repo_relative_path}"],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            raise BackendError("Failed to load cube revision", status=500) from exc
        return result.stdout or ""


def _log_cube_library_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured cube revision diagnostic line in standard Comfy logs."""

    log_diagnostic(_logger, CUBE_REVISION_TRACE_MARKER, event, fields)
