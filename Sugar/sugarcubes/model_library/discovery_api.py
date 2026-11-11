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
"""Expose bounded discovery commands without accepting client transport or destination evidence."""

from __future__ import annotations

import json
import logging

import aiohttp
from aiohttp import web

from .application.enrichment_service import ModelPreviewUnavailableError
from .domain.model_families import picker_suggestion_context
from .domain.model_suggestions import ModelSuggestionContext
from .domain.model_relationships import ModelRelationships
from .infrastructure.discovery_requests import ModelDiscoveryRequests
from .infrastructure.thumbnail_assets import ModelThumbnailAssets

_LOGGER = logging.getLogger(__name__)
_BASE = "/sugarcubes/v1/models/discovery"


def register_model_discovery_routes(
    routes: web.RouteTableDef,
    discovery: ModelDiscoveryRequests,
    assets: ModelThumbnailAssets,
    relationships: ModelRelationships,
) -> None:
    """Translate Vue commands into the shared reviewed-plan and observable-job owners."""

    async def context(request: web.Request) -> web.Response:
        """Resolve an explicit cube family without contacting providers or guessing from node names."""
        try:
            value = await _payload(request, {"kind", "targetModel"})
            resolved = _context(value, relationships)
        except ValueError:
            return _error("Invalid model discovery context.", 400)
        return web.json_response(
            {
                "schemaVersion": 1,
                "context": (
                    {
                        "artifactKind": resolved.artifact_kind,
                        "familyId": resolved.family_id,
                    }
                    if resolved is not None
                    else None
                ),
            },
            headers={"Cache-Control": "no-store"},
        )

    async def discover(request: web.Request) -> web.Response:
        """Start cancellable provider planning after validating the reference's supported context."""
        try:
            value = await _payload(request, {"kind", "targetModel", "publicOnly"})
            resolved = _context(value, relationships)
            public_only = value.get("publicOnly", False)
            if resolved is None or not isinstance(public_only, bool):
                raise ValueError("Unsupported discovery context or access policy.")
        except ValueError:
            return _error("Invalid or unsupported model discovery request.", 400)
        return web.json_response(
            discovery.discover(resolved, public_only=public_only).to_payload(),
            status=202,
            headers={"Cache-Control": "no-store"},
        )

    async def acquire(request: web.Request) -> web.Response:
        """Accept only a retained card identity and one reviewed provider choice."""
        try:
            value = await _payload(request, {"identity", "providerId"})
            identity, provider = value.get("identity"), value.get("providerId")
            if (
                not isinstance(identity, str)
                or not 1 <= len(identity) <= 160
                or not isinstance(provider, str)
                or not 1 <= len(provider) <= 64
            ):
                raise ValueError("Invalid model suggestion selection.")
            job = await discovery.acquire(
                request.match_info["plan_id"], identity, provider
            )
        except ValueError as error:
            return _error(str(error), 400)
        return web.json_response(
            job.to_payload(), status=202, headers={"Cache-Control": "no-store"}
        )

    async def preview(request: web.Request) -> web.StreamResponse:
        """Serve a prepared immutable image for a retained offer without accepting remote URLs."""
        try:
            images = await discovery.preview(
                request.match_info["plan_id"],
                int(request.match_info["index"]),
                request.match_info["provider_id"],
            )
            image = max(
                (item for item in images if item.role == "standard"),
                key=lambda item: item.width,
                default=None,
            )
            asset = assets.resolve(image.asset_id) if image is not None else None
        except (
            aiohttp.ClientError,
            TimeoutError,
            ValueError,
            OSError,
            ModelPreviewUnavailableError,
        ) as error:
            _LOGGER.warning(
                "Model discovery preview unavailable; errorType=%s",
                type(error).__name__,
            )
            return _error("Model preview is unavailable.", 404)
        if asset is None:
            return _error("Model preview is unavailable.", 404)
        return web.FileResponse(
            asset.path,
            headers={
                "Content-Type": asset.content_type,
                "Cache-Control": "public, max-age=3600",
                "X-Content-Type-Options": "nosniff",
            },
        )

    async def release(request: web.Request) -> web.Response:
        """Release closed-presentation evidence without modifying shared acquisition jobs."""
        discovery.release(request.match_info["plan_id"])
        return web.Response(status=204)

    session = _BASE + "/{plan_id:[a-f0-9]{32}}"
    routes.post(_BASE + "/context")(context)
    routes.post(_BASE)(discover)
    routes.post(session + "/acquire")(acquire)
    routes.get(session + "/preview/{index:[0-9]{1,3}}/{provider_id}")(preview)
    routes.delete(session)(release)


async def _payload(request: web.Request, allowed: set[str]) -> dict[str, object]:
    """Bound and validate external JSON before it reaches reviewed-plan orchestration."""
    content = bytearray()
    async for block in request.content.iter_chunked(4096):
        if len(content) + len(block) > 16 * 1024:
            raise ValueError("Model discovery request exceeds its bound.")
        content.extend(block)
    value: object = json.loads(content)
    if not isinstance(value, dict) or set(value) - allowed:
        raise ValueError("Invalid model discovery request.")
    return {str(key): item for key, item in value.items()}


def _context(
    value: dict[str, object], relationships: ModelRelationships
) -> ModelSuggestionContext | None:
    """Delegate family eligibility to the same domain owner used by discovery providers."""
    kind, target = value.get("kind"), value.get("targetModel", "")
    if (
        not isinstance(kind, str)
        or not 1 <= len(kind) <= 64
        or not isinstance(target, str)
        or len(target) > 128
    ):
        raise ValueError("Invalid model discovery context.")
    return picker_suggestion_context(kind, target, relationships)


def _error(message: str, status: int) -> web.Response:
    """Return recoverable command feedback without launching a blocking dialog."""
    return web.json_response(
        {"error": message}, status=status, headers={"Cache-Control": "no-store"}
    )
