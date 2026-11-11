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
"""Own Cube library catalog state, status, and change events."""

from __future__ import annotations

import logging
from collections.abc import Callable, Sequence
from typing import Any, Mapping

from ..responses import BackendError
from .cube_catalog_state_service import CubeCatalogStateService
from .cube_library_catalog_projection import CubeLibraryCatalogProjection
from .cube_library_diagnostics import (
    log_cube_library_diagnostic,
    runtime_version,
    utc_now,
)
from .cube_library_listing import CubeLibraryListing
from .cube_library_source_resolver import CubeLibrarySourceResolver
from ...workflow_node_pack import current_workflow_node_pack

_logger = logging.getLogger(__name__)


class CubeLibraryStateService:
    """Own cached catalog state and library-wide change notification."""

    def __init__(
        self,
        *,
        listing: CubeLibraryListing,
        projection: CubeLibraryCatalogProjection,
        sources: CubeLibrarySourceResolver,
    ) -> None:
        """Initialize catalog state from explicit listing and projection owners."""

        self._sources = sources
        self._listeners: list[Callable[[dict[str, Any]], None]] = []
        self._catalog_state = CubeCatalogStateService(
            list_summaries=lambda include_disabled: listing.list_catalog_cube_summaries(
                include_disabled=include_disabled
            ),
            build_entry=projection.catalog_entry_for_summary,
            revision_pack_facts=lambda include_disabled: projection.revision_pack_facts(
                include_disabled=include_disabled
            ),
            pack_counts=projection.pack_counts,
            generated_at=utc_now,
        )

    def subscribe(
        self, listener: Callable[[dict[str, Any]], None]
    ) -> Callable[[], None]:
        """Register a generic library-change listener and return an unsubscribe."""

        self._listeners.append(listener)

        def unsubscribe() -> None:
            """Remove the registered library-change listener."""

            try:
                self._listeners.remove(listener)
            except ValueError:
                return

        return unsubscribe

    def notify(
        self,
        *,
        affected_cube_ids: Sequence[str],
        saved_versions: Mapping[str, str],
        reason: str,
    ) -> None:
        """Publish a generic library-change event to in-process consumers."""

        self.invalidate(reason=reason, affected_cube_ids=affected_cube_ids)
        event = {
            "schemaVersion": 1,
            "affectedCubeIds": list(affected_cube_ids),
            "savedVersions": dict(saved_versions),
            "generatedAt": utc_now(),
            "reason": reason,
            "catalogRevision": self.catalog_revision(),
        }
        for listener in tuple(self._listeners):
            try:
                listener(dict(event))
            except (RuntimeError, TypeError, ValueError):
                _logger.exception("SugarCubes library change listener failed")

    def invalidate(self, *, reason: str, affected_cube_ids: Sequence[str] = ()) -> None:
        """Invalidate cached catalog and source state after a visible mutation."""

        self._catalog_state.invalidate(reason, affected_cube_ids=affected_cube_ids)
        self._sources.invalidate()

    def library_status(self) -> dict[str, Any]:
        """Return target-owned Cube Library availability for backend adapters."""

        try:
            catalog_revision = self.catalog_revision()
            available = True
            errors: list[dict[str, str]] = []
        except BackendError as exc:
            available = False
            catalog_revision = ""
            errors = [{"code": "catalog-unavailable", "message": exc.message}]
        except (OSError, RuntimeError, TypeError, ValueError):
            _logger.exception("SugarCubes: library status failed")
            available = False
            catalog_revision = ""
            errors = [
                {
                    "code": "catalog-unavailable",
                    "message": "SugarCubes catalog is unavailable.",
                }
            ]
        return self._status_payload(
            available=available,
            catalog_revision=catalog_revision,
            errors=errors,
        )

    def capabilities_status(self) -> dict[str, Any]:
        """Return Cube Library capability facts without building catalog state."""

        return self._status_payload(available=True, catalog_revision="", errors=[])

    def catalog_revision(self, *, include_disabled: bool = False) -> str:
        """Return a deterministic revision for catalog-relevant library state."""

        revision = self._catalog_state.current_revision(
            include_disabled=include_disabled
        )
        log_cube_library_diagnostic(
            "sugarcubes_catalog_revision",
            include_disabled=include_disabled,
            catalog_revision=revision,
        )
        return revision

    def list_catalog(self, *, include_disabled: bool = False) -> dict[str, Any]:
        """Return backend-facing catalog metadata for enabled library cubes."""

        log_cube_library_diagnostic(
            "sugarcubes_list_catalog_start", include_disabled=include_disabled
        )
        payload = self._catalog_state.current_catalog(include_disabled=include_disabled)
        log_cube_library_diagnostic(
            "sugarcubes_list_catalog_return",
            include_disabled=include_disabled,
            cube_count=len(payload["cubes"]),
            catalog_revision=payload["catalogRevision"],
        )
        return payload

    @staticmethod
    def _status_payload(
        *,
        available: bool,
        catalog_revision: str,
        errors: list[dict[str, str]],
    ) -> dict[str, Any]:
        """Build the stable host-facing library status response."""

        node_pack = current_workflow_node_pack()
        return {
            "schemaVersion": 1,
            "available": available,
            "source": "SugarCubes",
            "sugarCubesVersion": runtime_version(),
            "workflowNodePack": node_pack.api_payload(),
            "catalogRevision": catalog_revision,
            "packManagementSupported": available,
            "localAuthoringSupported": available,
            "readinessSupported": available,
            "dependencyReadinessSupported": available,
            "dependencyRepairSupported": available,
            "versionedDependencyReadinessSupported": available,
            "syncDependencyOrchestrationSupported": available,
            "errors": errors,
        }
