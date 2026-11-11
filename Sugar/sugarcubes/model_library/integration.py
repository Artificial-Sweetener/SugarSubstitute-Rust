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
"""Bind the model library to the running Comfy server at one host boundary."""

from __future__ import annotations

from aiohttp import web

from .api import register_model_library_routes
from .metadata_api import register_model_metadata_routes
from .download_api import register_model_download_routes
from sugarcubes_model_roots import ModelRootConfiguration
from .root_api import register_model_root_routes
from .release_api import register_model_release_routes
from .update_api import register_model_update_routes
from .detector_api import register_detector_routes
from .fingerprint_api import register_model_fingerprint_routes
from .credential_api import register_model_credential_routes
from .discovery_api import register_model_discovery_routes
from .preference_api import register_model_preference_routes
from .cache_api import register_model_cache_routes
from .infrastructure.comfy_roots import ComfyModelRoots
from .infrastructure.runtime import ModelLibraryRuntime
from .infrastructure.comfy_model_usage import ComfyModelUsageObserver
from .infrastructure.comfy_credentials import host_civitai_credential_store
from .host_api import (
    active_model_library,
    model_library_for_host,
    publish_model_library,
    release_model_library,
)


def install_model_library(prompt_server: object) -> ModelLibraryRuntime:
    """Validate the host route table and compose one authoritative library runtime."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    routes: object = getattr(instance, "routes", None)
    if not isinstance(routes, web.RouteTableDef):
        raise RuntimeError("Comfy's HTTP route registry is unavailable.")
    host_app: object = getattr(instance, "app", None)
    if not isinstance(host_app, web.Application):
        raise RuntimeError("Comfy's HTTP application lifecycle is unavailable.")
    current = model_library_for_host(host_app)
    if current is not None:
        return current
    if active_model_library() is not None:
        raise RuntimeError(
            "SugarCubes model library is already bound to another Comfy host."
        )
    runtime = ModelLibraryRuntime(
        ComfyModelRoots.from_host(), credential_store=host_civitai_credential_store()
    )
    usage_observer = ComfyModelUsageObserver(
        getattr(instance, "prompt_queue", None), runtime.queued_model_usage
    )
    register_model_library_routes(routes, runtime)
    register_model_download_routes(routes, runtime.download_preferences)
    register_model_metadata_routes(
        routes, runtime.relationships, runtime.content_classification, runtime.updates
    )
    register_model_release_routes(routes, runtime)
    register_model_update_routes(routes, runtime.update_feed)
    register_model_preference_routes(routes, runtime.preferences)
    register_model_cache_routes(routes, runtime)
    register_detector_routes(routes, runtime.detectors, runtime.updates)
    register_model_fingerprint_routes(routes, runtime)
    register_model_credential_routes(routes, runtime.credentials)
    register_model_discovery_routes(
        routes, runtime.discovery, runtime.assets, runtime.relationships
    )
    register_model_root_routes(routes, ModelRootConfiguration.from_host())

    async def start_library(application: web.Application) -> None:
        """Queue stat and local-preview preparation without waiting on model contents."""
        runtime.watcher.start(runtime.start_initial_scan())

    async def close_library(application: web.Application) -> None:
        """Cancel owned background work when the host HTTP application shuts down."""
        await runtime.watcher.close()
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()
        await runtime.update_feed.close()
        await runtime.releases.history_cache.close()
        await runtime.release_previews.close()
        usage_observer.close()
        release_model_library(runtime)

    host_app.on_cleanup.append(close_library)
    host_app.on_startup.append(start_library)
    publish_model_library(host_app, runtime)
    return runtime
