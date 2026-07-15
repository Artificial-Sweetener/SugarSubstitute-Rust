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
"""Own current and historical cube artifact selection and cache warming."""

from __future__ import annotations

import json
import logging
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from time import perf_counter
from typing import Any, Mapping, Protocol

from ...instrumentation import log_diagnostic
from ..responses import BackendError
from .cube_file_io import (
    compute_cube_content_hash,
    compute_cube_content_hash_bytes,
    format_display_path,
    format_timestamp,
    read_cube_payload,
)
from .cube_git_context import CubeGitContext, resolve_cube_git_context
from .cube_metadata import (
    _mapping_list,
    normalize_metadata_string,
    normalize_supported_models,
)
from .cube_summary import build_cube_identity_fields, derive_cube_display_name
from .cube_version_artifact_cache import (
    CubeVersionArtifactCache,
    CubeVersionArtifactCacheKey,
    CubeVersionSelectionCacheKey,
)
from .tracked_repo_service import TrackedRepo, TrackedRepoService

_logger = logging.getLogger(__name__)
_ARTIFACT_TRACE_MARKER = "SugarCubes cube library diagnostic"
_CURRENT_REVISION_REF = "WORKTREE"


class CubeLibraryArtifactOwner(Protocol):
    """Describe the library operations required by artifact selection."""

    extension_root: Path
    load_cube_artifact: Any
    tracked_repo_service: TrackedRepoService

    def resolve_cube_by_id(self, cube_id: str) -> Path: ...

    def summarize_cube(self, cube_path: Path) -> dict[str, Any]: ...

    def _source_metadata_for_summary(
        self,
        summary: Mapping[str, Any],
        *,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None = None,
    ) -> dict[str, Any]: ...

    def _local_head_sha(self, tracked: TrackedRepo) -> str: ...

    def _is_repo_path_dirty(self, checkout: Path, relative_path: str) -> bool: ...

    def _local_source_relative_path(self, cube_id: str) -> str: ...

    def _local_source_metadata(
        self,
        *,
        namespace: str,
        source_path: str,
        repo_root: Path,
        repo_relative_path: str,
    ) -> dict[str, Any]: ...


def _runtime_version() -> str:
    """Return the SugarCubes runtime version exposed to backend adapters."""

    from .. import __version__

    return normalize_metadata_string(__version__)


def _log_artifact_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured cube artifact diagnostic."""

    log_diagnostic(_logger, _ARTIFACT_TRACE_MARKER, event, fields)


class CubeLibraryArtifactService:
    """Load and select exact current or historical cube artifacts."""

    def __init__(self, library: CubeLibraryArtifactOwner) -> None:
        """Initialize artifact selection and its durable cache."""

        self._library = library
        self.version_artifact_cache = CubeVersionArtifactCache(
            library.extension_root / ".sugarcubes" / "cache" / "cube-version-artifacts"
        )
        self._version_warm_executor = ThreadPoolExecutor(
            max_workers=1,
            thread_name_prefix="sugarcubes-version-warm",
        )
        try:
            self.version_artifact_cache.prune()
        except (OSError, RuntimeError, TypeError, ValueError):
            _logger.warning(
                "SugarCubes: version artifact cache prune failed during startup",
                exc_info=True,
            )

    def load_library_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the canonical cube document and source metadata for one cube id."""

        started_at = perf_counter()
        _log_artifact_diagnostic(
            "sugarcubes_load_library_cube_start",
            requested_cube_id=cube_id,
            version_ref_supported=False,
            revision_ref_supported=False,
        )
        context = self._resolve_cube_ref_context(cube_id)
        response = self._load_current_library_artifact(context)
        _log_artifact_diagnostic(
            "sugarcubes_load_library_cube_return",
            requested_cube_id=cube_id,
            loaded_cube_id=response["cubeId"],
            loaded_version=response["version"],
            content_hash=response["contentHash"],
            cube_path=format_display_path(context.cube_path, self._library.extension_root),
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
            _log_artifact_diagnostic(
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
        _log_artifact_diagnostic(
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

    def _resolve_cube_ref_context(self, cube_id: str) -> CubeGitContext:
        """Resolve a cube ref context and verify the current file exists."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        context = resolve_cube_git_context(self._library.tracked_repo_service, normalized)
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
        summary = self._library.summarize_cube(context.cube_path)
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
            "source": self._library._source_metadata_for_summary(summary),
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
            result = self._library.tracked_repo_service.git_runner(
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

        refs = _mapping_list(
            self.list_library_cube_refs(context.cube_id).get("refs")
        )
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

        refs = _mapping_list(
            self.list_library_cube_refs(context.cube_id).get("refs")
        )
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
                    _log_artifact_diagnostic(
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
            result = self._library.tracked_repo_service.git_runner(
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
            tracked = self._library.tracked_repo_service.get_repo(context.owner, context.repo)
            return self._library._local_head_sha(tracked)
        if not (context.repo_root / ".git").exists():
            return "nogit"
        try:
            result = self._library.tracked_repo_service.git_runner(
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
            result = self._library.tracked_repo_service.git_runner(
                ["show", f"{revision_ref}:{context.repo_relative_path}"],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            raise BackendError("Failed to load cube revision", status=500) from exc
        return getattr(result, "stdout", "") or ""

    def _source_metadata_for_context(self, context: CubeGitContext) -> dict[str, Any]:
        """Build source metadata directly from resolved cube ownership context."""

        if context.source_kind == "github":
            tracked = self._library.tracked_repo_service.get_repo(context.owner, context.repo)
            return {
                "kind": "github",
                "repoRef": f"{context.owner}/{context.repo}",
                "owner": context.owner,
                "repo": context.repo,
                "branch": tracked.branch,
                "path": context.repo_relative_path,
                "localHeadSha": self._library._local_head_sha(tracked),
                "remoteHeadSha": tracked.remote_head_sha,
                "dirty": self._library._is_repo_path_dirty(
                    context.repo_root, context.repo_relative_path
                ),
            }
        return self._library._local_source_metadata(
            namespace=context.namespace,
            source_path=context.repo_relative_path,
            repo_root=context.repo_root,
            repo_relative_path=context.repo_relative_path,
        )
