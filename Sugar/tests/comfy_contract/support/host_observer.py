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
"""Expose read-only host facts to the disposable real-host test installation."""

from __future__ import annotations

import inspect
import sys
from collections import defaultdict
from types import CodeType
from typing import Any

from aiohttp import web

NODE_CLASS_MAPPINGS: dict[str, type[object]] = {}


class PackExecution:
    """Observe executed pack lines during each scenario in the disposable interpreter."""

    def __init__(self) -> None:
        """Defer monitoring until an explicit request from the test harness."""
        self.tool: int | None = None
        self.lines: dict[str, set[int]] = defaultdict(set)
        self.paths: dict[CodeType, str] = {}

    def begin(self) -> None:
        """Select only SugarCubes code objects for per-line events across host threads."""
        monitor = sys.monitoring
        if self.tool is None:
            tool = next(
                index for index in (3, 4, 2, 1, 0, 5) if monitor.get_tool(index) is None
            )
            monitor.use_tool_id(tool, "SugarCubes compatibility")
            self.tool = tool

            def enter(code: CodeType, offset: int) -> object:
                """Enable local events on product code and stop scanning other code objects."""
                _, found, path = code.co_filename.replace("\\", "/").partition(
                    "/custom_nodes/SugarCubes/"
                )
                if found:
                    self.paths[code] = path
                    monitor.set_local_events(tool, code, monitor.events.LINE)
                return monitor.DISABLE

            def line(code: CodeType, number: int) -> None:
                """Retain unique source lines reached during this scenario."""
                self.lines[self.paths[code]].add(number)

            monitor.register_callback(tool, monitor.events.PY_START, enter)
            monitor.register_callback(tool, monitor.events.LINE, line)
            monitor.set_events(tool, monitor.events.PY_START)
        self.lines.clear()

    def snapshot(self) -> dict[str, list[int]]:
        """Freeze the window before serializing concurrent worker observations."""
        recorded = self.lines
        self.lines = defaultdict(set)
        return {path: sorted(lines) for path, lines in sorted(recorded.items())}


def observe(instance: Any, registry: dict[str, Any]) -> dict[str, object]:
    """Snapshot actual registrations, native node contracts and queue collaborators."""
    routes = sorted(
        {
            f"{route.method} {route.resource.canonical}"
            for route in instance.app.router.routes()
            if route.resource
            and route.resource.canonical.startswith("/sugarcubes/")
            and route.method != "HEAD"
        }
    )
    definitions = {}
    for name, constructor in registry.items():
        if name.startswith("SugarCubes."):
            inputs = constructor.INPUT_TYPES()
            definitions[name] = {
                "required": sorted(inputs.get("required", {})),
                "optional": sorted(inputs.get("optional", {})),
                "outputs": list(constructor.RETURN_TYPES),
                "method": constructor.FUNCTION,
                "outputNode": getattr(constructor, "OUTPUT_NODE", False),
            }
    targets = {
        "execution": sys.modules["execution"],
        "server": instance,
        "queue": instance.prompt_queue,
        "replacements": getattr(instance, "node_replace_manager", None),
        "folder_paths": sys.modules["folder_paths"],
    }
    return {
        "routes": routes,
        "nodeDefinitions": definitions,
        "members": {
            label: sorted(
                key
                for key in dir(value)
                if not key.startswith("__")
                and inspect.getattr_static(value, key, None) is not None
            )
            for label, value in targets.items()
        },
    }


def install() -> None:
    """Register the isolated observer after the host has constructed its application."""
    server = getattr(sys.modules["server"], "PromptServer").instance

    async def read(request: web.Request) -> web.Response:
        """Resolve facts at request time, after the host has loaded all custom nodes."""
        return web.json_response(
            observe(server, getattr(sys.modules["nodes"], "NODE_CLASS_MAPPINGS"))
        )

    server.routes.get("/sugar-test-host/facts")(read)
    execution = PackExecution()

    async def begin_coverage(request: web.Request) -> web.Response:
        """Open the measurement window for the single active scenario."""
        execution.begin()
        return web.json_response({"recording": True})

    async def end_coverage(request: web.Request) -> web.Response:
        """Return the observed backend sources and line numbers."""
        return web.json_response(execution.snapshot())

    server.routes.post("/sugar-test-host/coverage/start")(begin_coverage)
    server.routes.get("/sugar-test-host/coverage")(end_coverage)


install()
