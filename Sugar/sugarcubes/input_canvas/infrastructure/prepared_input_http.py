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
"""Adapt processed input assets to the existing execution-owned graph projection."""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping
import logging

from aiohttp import web

from ...backend.responses import json_error
from ...execution.limits import MAX_NATIVE_SCOPE_NODES
from ...execution.models import ApiPrompt
from ...execution.prepared_input_values import (
    PreparedInputAsset,
    parse_prepared_input_assets,
)
from .comfy_media_path import validate_input_file

PreparedInputPrompt = Callable[[ApiPrompt, tuple[PreparedInputAsset, ...]], ApiPrompt]
_logger = logging.getLogger(__name__)
_INSTALLED = web.AppKey("sugarcubes.prepared_inputs", bool)


def install_prepared_input_route(
    prompt_server: object, prepare: PreparedInputPrompt
) -> None:
    """Install one normal-prompt adapter to the same owner used by direct Cube execution."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    application: object = getattr(instance, "app", None)
    routes: object = getattr(instance, "routes", None)
    if not isinstance(application, web.Application) or not isinstance(
        routes, web.RouteTableDef
    ):
        raise RuntimeError("Comfy's prepared-input HTTP surface is unavailable.")
    if application.get(_INSTALLED):
        return

    async def prepared_inputs(request: web.Request) -> web.Response:
        """Validate transport and owned uploads before requesting a derived execution graph."""
        try:
            payload: object = await request.json()
            if not isinstance(payload, Mapping):
                raise ValueError("Invalid prepared input request.")
            assets = parse_prepared_input_assets(payload.get("assets"))
            prompt = _prompt(payload.get("prompt"))
            for asset in assets:
                validate_input_file(asset.prepared_file)
            derived = await asyncio.to_thread(prepare, prompt, assets)
            return web.json_response(
                {"prompt": derived}, headers={"Cache-Control": "no-store"}
            )
        except FileNotFoundError as error:
            return web.json_response({"error": str(error)}, status=404)
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except Exception:
            # Installed schema providers can raise native exceptions at this adapter boundary.
            _logger.exception("SugarCubes prepared input projection failed")
            return json_error("Input preparation is unavailable.", status=500)

    routes.post("/sugarcubes/v2/canvas-inputs/prepare")(prepared_inputs)
    application[_INSTALLED] = True


def _prompt(value: object) -> ApiPrompt:
    """Narrow a bounded canonical prompt without interpreting any node behavior at the route boundary."""
    if not isinstance(value, Mapping) or len(value) > MAX_NATIVE_SCOPE_NODES:
        raise ValueError("Invalid prepared input prompt.")
    result: ApiPrompt = {}
    for node_id, raw in value.items():
        if not isinstance(node_id, str) or not node_id or not isinstance(raw, Mapping):
            raise ValueError("Invalid prepared input node.")
        if any(not isinstance(key, str) for key in raw):
            raise ValueError("Invalid prepared input node fields.")
        node: dict[str, object] = {
            key: item for key, item in raw.items() if isinstance(key, str)
        }
        inputs = node.get("inputs")
        if (
            not isinstance(node.get("class_type"), str)
            or not isinstance(inputs, Mapping)
            or any(not isinstance(key, str) for key in inputs)
        ):
            raise ValueError("Invalid prepared input node.")
        result[node_id] = node
    return result
