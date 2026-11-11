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
"""Expose secret-free credential status and explicit commands through the shared service."""

from __future__ import annotations

import json
import logging

from aiohttp import web

from .application.credential_service import CivitaiCredentialService

_LOGGER = logging.getLogger(__name__)


def register_model_credential_routes(
    routes: web.RouteTableDef, credentials: CivitaiCredentialService
) -> None:
    """Keep secure persistence and provider validation out of HTTP request handlers."""

    async def status(request: web.Request) -> web.Response:
        """Return capability and configured state with no credential value or path."""
        return web.json_response(
            await credentials.status(), headers={"Cache-Control": "no-store"}
        )

    async def save(request: web.Request) -> web.Response:
        """Persist one explicit validated key without reflecting it into responses or logs."""
        try:
            key = await _candidate(request, required=True)
            if key is None:
                raise ValueError("Missing credential.")
            await credentials.save(key)
        except ValueError:
            return _error("The CivitAI credential could not be saved.", 400)
        except OSError as error:
            _LOGGER.warning(
                "Secure CivitAI credential save failed; errorType=%s",
                type(error).__name__,
            )
            return _error("The CivitAI credential could not be saved.", 500)
        return web.json_response(
            await credentials.status(), headers={"Cache-Control": "no-store"}
        )

    async def clear(request: web.Request) -> web.Response:
        """Remove stored material while preserving any explicit environment override."""
        try:
            await credentials.clear()
        except OSError as error:
            _LOGGER.warning(
                "Secure CivitAI credential removal failed; errorType=%s",
                type(error).__name__,
            )
            return _error("The CivitAI credential could not be removed.", 500)
        return web.json_response(
            await credentials.status(), headers={"Cache-Control": "no-store"}
        )

    async def test(request: web.Request) -> web.Response:
        """Validate an entered or configured key without persisting entered material."""
        try:
            result = await credentials.test(await _candidate(request, required=False))
        except ValueError:
            return _error("The CivitAI credential is invalid.", 400)
        return web.json_response(
            {
                "schemaVersion": 1,
                "succeeded": result.succeeded,
                "message": result.message,
            },
            headers={"Cache-Control": "no-store"},
        )

    path = "/sugarcubes/v1/models/credentials/civitai"
    routes.get(path)(status)
    routes.put(path)(save)
    routes.delete(path)(clear)
    routes.post(path + "/test")(test)


async def _candidate(request: web.Request, *, required: bool) -> str | None:
    """Decode one bounded JSON field without accepting arbitrary keys or destinations."""
    content = bytearray()
    async for block in request.content.iter_chunked(4096):
        if len(content) + len(block) > 16 * 1024:
            raise ValueError("Credential request exceeds its bound.")
        content.extend(block)
    value: object = json.loads(content)
    if not isinstance(value, dict) or set(value) - {"apiKey"}:
        raise ValueError("Invalid credential request.")
    key: object = value.get("apiKey")
    if key is None and not required:
        return None
    if not isinstance(key, str):
        raise ValueError("Invalid credential request.")
    return key


def _error(message: str, status: int) -> web.Response:
    """Keep all credential failures generic and explicitly noncacheable."""
    return web.json_response(
        {"error": message}, status=status, headers={"Cache-Control": "no-store"}
    )
