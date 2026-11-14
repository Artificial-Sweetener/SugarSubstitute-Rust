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
"""Adapt SugarCubes' release notes to HTTP for the page's What's new view.

The shapes are described in `release_notes.wire`. A notes read runs off the
event loop through the bounded release client, so a slow network never stalls
ComfyUI's server.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any

from .composition import BackendServices
from .pack_install.errors import PackInstallError
from .pack_install.pack_identity import parse_release_version
from .release_notes import notes_to_wire, version_to_wire
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler


@dataclass(frozen=True)
class ReleaseNotesRouteHandlers:
    """Collect the release notes endpoint handlers for registration."""

    get_release_notes_version: RouteHandler
    get_release_notes: RouteHandler


def build_release_notes_route_handlers(
    services: BackendServices,
) -> ReleaseNotesRouteHandlers:
    """Build thin handlers over the release notes service."""

    notes = services.release_notes

    async def get_release_notes_version(request: Any) -> Any:
        _ = request
        return json_success(version_to_wire(notes.current_version))

    async def get_release_notes(request: Any) -> Any:
        try:
            since = parse_release_version(request.query.get("since"))
        except PackInstallError:
            return json_error_from_exception(
                BackendError("Name the version the notes start after.", status=400)
            )
        outcome = await asyncio.to_thread(notes.notes_since, since)
        return json_success(notes_to_wire(outcome))

    return ReleaseNotesRouteHandlers(
        get_release_notes_version=get_release_notes_version,
        get_release_notes=get_release_notes,
    )


__all__ = ["ReleaseNotesRouteHandlers", "build_release_notes_route_handlers"]
