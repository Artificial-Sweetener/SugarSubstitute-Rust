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
"""Bind shared output saving to the running Comfy application and queue lifecycle."""

from __future__ import annotations

from aiohttp import web

from .host_binding import bind_runtime_images, release_runtime_images
from .infrastructure.http_routes import managed_image_middleware, register_saving_routes
from .infrastructure.queue_lifecycle import SavingQueueLifecycle
from .infrastructure.runtime import OutputSavingRuntime
from .infrastructure.native_prompt_hook import NativeSavingPromptHook

_RUNTIME = web.AppKey("sugarcubes.output_saving", OutputSavingRuntime)


def install_output_saving(prompt_server: object) -> OutputSavingRuntime:
    """Compose one runtime per host and preserve native routes and queue observers."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    application: object = getattr(instance, "app", None)
    routes: object = getattr(instance, "routes", None)
    queue: object = getattr(instance, "prompt_queue", None)
    if (
        not isinstance(application, web.Application)
        or not isinstance(routes, web.RouteTableDef)
        or queue is None
    ):
        raise RuntimeError(
            "Comfy's output saving application and queue surfaces are unavailable."
        )
    existing = application.get(_RUNTIME)
    if existing is not None:
        return existing
    runtime = OutputSavingRuntime.from_host()
    add_handler: object = getattr(instance, "add_on_prompt_handler", None)
    if not callable(add_handler):
        raise RuntimeError("Comfy's native prompt preparation hook is unavailable.")
    hook = NativeSavingPromptHook(runtime.policy)
    observer = SavingQueueLifecycle(queue, runtime.policy)
    installed = False
    try:
        runtime.policy.retain_active(observer.live_ids())
        bind_runtime_images(runtime.images)
        add_handler(hook)
        register_saving_routes(routes, runtime)
        application.middlewares.append(managed_image_middleware(runtime))
        installed = True
    finally:
        if not installed:
            observer.close()
            hook.close()
            release_runtime_images(runtime.images)

    async def close_runtime(app: web.Application) -> None:
        """Release owned queue hooks and node bindings without deleting saved images or preferences."""
        observer.close()
        hook.close()
        release_runtime_images(runtime.images)

    async def stop_artifacts(app: web.Application) -> None:
        """Cancel and observe artifact workers before the server releases its application lifetime."""
        await runtime.artifact_workers.close()

    application.on_shutdown.append(stop_artifacts)
    application.on_cleanup.append(close_runtime)
    application[_RUNTIME] = runtime
    return runtime
