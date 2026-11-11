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
"""Adapt bounded spatial-probe requests to the installed Comfy runtime."""

from __future__ import annotations

from collections.abc import Awaitable, Callable, Mapping
import json
import logging
import math
import sys
from typing import cast
from uuid import UUID

from aiohttp import web

from ..backend.responses import json_error
from .comfy_execution_port import (
    ComfyExecutionModuleLike,
    DirectComfyExecutionPort,
    PromptServerLike,
)
from .comfy_spatial_probe import ComfySpatialProbe, SpatialNativeQueue
from .models import ApiPrompt
from .spatial_probe_graph import SpatialProbeGraph
from .spatial_probe_registry import SpatialProbeRegistry
from .spatial_probe_service import SpatialProbeService
from .spatial_probe_values import SpatialFieldTarget

_logger = logging.getLogger(__name__)
_INSTALLED = web.AppKey("sugarcubes.spatial_probe", bool)
_MAX_BYTES = 8 * 1024 * 1024


def _registry() -> Mapping[str, object]:
    """Read the active registry lazily after all normal custom nodes are registered."""
    value: object = getattr(sys.modules.get("nodes"), "NODE_CLASS_MAPPINGS", None)
    if not isinstance(value, Mapping):
        raise RuntimeError("The active Comfy node registry is unavailable.")
    return {key: item for key, item in value.items() if isinstance(key, str)}


def _json_value(value: object, depth: int = 0) -> object:
    """Bound nesting and reject non-JSON or nonfinite data before domain admission."""
    if depth > 32:
        raise ValueError("The probe graph is too deeply nested.")
    if value is None or type(value) in (str, bool, int):
        return value
    if isinstance(value, float) and math.isfinite(value):
        return value
    if isinstance(value, list):
        return [_json_value(item, depth + 1) for item in value]
    if isinstance(value, Mapping) and all(isinstance(key, str) for key in value):
        return {str(key): _json_value(item, depth + 1) for key, item in value.items()}
    raise ValueError("The probe graph contains invalid JSON values.")


def _prompt(value: object) -> ApiPrompt:
    """Admit an actual compiled prompt without accepting native execution-only measurement nodes."""
    if not isinstance(value, Mapping) or not value or len(value) > 4096:
        raise ValueError("Invalid compiled spatial probe graph.")
    result: ApiPrompt = {}
    for key, node in value.items():
        if (
            not isinstance(key, str)
            or not key
            or len(key) > 512
            or not isinstance(node, Mapping)
        ):
            raise ValueError("Invalid compiled probe node.")
        name: object = node.get("class_type")
        inputs: object = node.get("inputs")
        if (
            not isinstance(name, str)
            or not name
            or len(name) > 256
            or name.startswith("SugarCubes.SpatialProbe")
            or not isinstance(inputs, Mapping)
        ):
            raise ValueError("Invalid spatial probe class or inputs.")
        result[key] = {"class_type": name, "inputs": _json_value(inputs)}
        meta: object = node.get("_meta")
        title: object = meta.get("title") if isinstance(meta, Mapping) else None
        if isinstance(title, str):
            result[key]["_meta"] = {"title": title[:256]}
    return result


def _targets(value: object) -> tuple[SpatialFieldTarget, ...]:
    """Narrow projected field addresses independently of presentation labels."""
    if not isinstance(value, list) or not value or len(value) > 64:
        raise ValueError("The dimension field has no bounded execution targets.")
    result: list[SpatialFieldTarget] = []
    for target in value:
        if not isinstance(target, Mapping):
            raise ValueError("Invalid dimension field target.")
        node, name = target.get("nodeId"), target.get("inputName")
        if (
            not isinstance(node, str)
            or not node
            or len(node) > 512
            or not isinstance(name, str)
            or not name
            or len(name) > 128
        ):
            raise ValueError("Invalid dimension field target address.")
        result.append(SpatialFieldTarget(node, name))
    return tuple(result)


async def _body(request: web.Request) -> Mapping[str, object]:
    """Enforce the body budget while streaming rather than allocating an unbounded payload first."""
    data = bytearray()
    async for chunk in request.content.iter_chunked(65536):
        if len(data) + len(chunk) > _MAX_BYTES:
            raise ValueError("The compiled spatial probe exceeds its request budget.")
        data.extend(chunk)
    value: object = json.loads(data)
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise ValueError("Invalid spatial probe request.")
    return {str(key): item for key, item in value.items()}


def install_spatial_probe_routes(prompt_server: object) -> None:
    """Compose the normal native queue adapter with measurement-only execution and thin routes."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    app: object = getattr(instance, "app", None)
    routes: object = getattr(instance, "routes", None)
    queue: object = getattr(instance, "prompt_queue", None)
    execution: object = sys.modules.get("execution")
    if (
        not isinstance(app, web.Application)
        or not isinstance(routes, web.RouteTableDef)
        or execution is None
    ):
        raise RuntimeError("The Comfy spatial probe runtime is unavailable.")
    if app.get(_INSTALLED):
        return
    if not all(
        callable(getattr(queue, name, None))
        for name in (
            "get_current_queue",
            "get_history",
            "delete_history_item",
            "delete_queue_item",
        )
    ):
        raise RuntimeError("The Comfy spatial probe queue is unavailable.")
    registry = SpatialProbeRegistry(_registry)
    # Comfy supplies these untyped native interfaces; the adapter above verifies its required queue surface.
    native = ComfySpatialProbe(
        cast(SpatialNativeQueue, queue),
        DirectComfyExecutionPort(
            prompt_server=cast(PromptServerLike, instance),
            execution_module=cast(ComfyExecutionModuleLike, execution),
        ),
        registry,
    )
    install_spatial_probe_handlers(
        app, routes, SpatialProbeService(SpatialProbeGraph(registry.contract), native)
    )


def install_spatial_probe_handlers(
    app: web.Application, routes: web.RouteTableDef, service: SpatialProbeService
) -> None:
    """Expose one captured-graph use case through bounded, authenticated-by-token local handles."""
    if app.get(_INSTALLED):
        return

    async def prepare(request: web.Request) -> web.Response:
        """Capture a detached graph and list its relevant image paths without execution."""
        body = await _body(request)
        token, plan = service.prepare(
            _prompt(body.get("prompt")),
            _targets(body.get("width_targets")),
            _targets(body.get("height_targets")),
        )
        return web.json_response(
            {
                "token": token,
                "outputs": [
                    {"id": item.id, "label": item.label} for item in plan.outputs
                ],
            },
            headers={"Cache-Control": "no-store"},
        )

    async def measure(request: web.Request) -> web.Response:
        """Run only the captured path explicitly chosen in the native sizing UI."""
        token = str(UUID(request.match_info["token"]))
        body = await _body(request)
        output: object = body.get("output")
        if not isinstance(output, str) or not output or len(output) > 512:
            raise ValueError("Choose a valid spatial output path.")
        result = await service.measure(token, output)
        return web.json_response(
            {
                "scale": result.scale,
                "inputs": result.input_sizes,
                "outputs": result.output_sizes,
            },
            headers={"Cache-Control": "no-store"},
        )

    async def release(request: web.Request) -> web.Response:
        """Release only the caller's captured snapshot, retaining accepted job cleanup."""
        service.release(str(UUID(request.match_info["token"])))
        return web.Response(status=204)

    def guarded(
        handler: Callable[[web.Request], Awaitable[web.Response]],
    ) -> Callable[[web.Request], Awaitable[web.Response]]:
        """Retain native exception context while reporting concise user-facing failures."""

        async def run(request: web.Request) -> web.Response:
            """Translate expected admission failures and unexpected installed-node errors."""
            try:
                return await handler(request)
            except ValueError as error:
                return web.json_response({"error": str(error)}, status=422)
            except TimeoutError as error:
                return web.json_response({"error": str(error)}, status=504)
            except Exception:
                # Native installed schema and execution providers can raise arbitrary host exceptions here.
                _logger.exception("Spatial probe native boundary failed")
                return json_error("The native spatial probe is unavailable.", status=500)

        return run

    prefix = "/sugarcubes/v2/executions/spatial"
    routes.post(prefix + "/prepare")(guarded(prepare))
    routes.post(prefix + "/{token}/measure")(guarded(measure))
    routes.delete(prefix + "/{token}")(guarded(release))
    app[_INSTALLED] = True
