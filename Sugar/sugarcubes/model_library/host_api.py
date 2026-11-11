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
"""Publish the running model authority for sibling Comfy extensions."""

from __future__ import annotations

import sys
from collections.abc import Sequence

from .infrastructure.runtime import ModelLibraryRuntime
from .infrastructure.model_update_payloads import model_update_snapshot
from .infrastructure.model_release_payloads import model_release_history_payload
from .infrastructure.provider_preference_codec import (
    provider_preference_patch,
    provider_preferences_payload,
)

MODEL_LIBRARY_HOST_API_VERSION = 1
MODEL_LIBRARY_HOST_API_MODULE = "sugarcubes.model_library.host_api"

_ACTIVE_LIBRARY: ModelLibraryRuntime | None = None
_ACTIVE_HOST: object | None = None


def prepared_model_provider_preferences() -> dict[str, object]:
    """Read shared provider policy without starting model or network work."""
    return provider_preferences_payload(_require_library().preferences.read())


def configure_model_provider_preferences(payload: object) -> dict[str, object]:
    """Merge validated host choices through the same owner used by browser settings."""
    patch = provider_preference_patch(payload)
    return provider_preferences_payload(_require_library().preferences.configure(patch))


def active_model_library() -> ModelLibraryRuntime | None:
    """Return the existing authority without constructing a second catalog or worker pool."""
    return _ACTIVE_LIBRARY


def prepared_model_catalog(kinds: Sequence[str] | None = None) -> dict[str, object]:
    """Read the shared indexed public catalog without scanning files or hashing weights."""
    library = active_model_library()
    if library is None:
        raise RuntimeError("SugarCubes model library is not active yet.")
    return library.catalog.list_models(kinds)


def model_catalog_revision() -> int:
    """Expose a cheap shared change token without catalog projection or filesystem discovery."""
    return _require_library().repository.revision()


def prepared_model_updates() -> dict[str, object]:
    """Read shared consent and proposals without starting provider traffic."""
    return model_update_snapshot(_require_library().update_feed)


def set_model_update_notifications(enabled: bool) -> dict[str, object]:
    """Persist explicit consent through the same owner used by browser settings."""
    if not isinstance(enabled, bool):
        raise ValueError("Model update consent must be boolean.")
    feed = _require_library().update_feed
    feed.set_enabled(enabled)
    return model_update_snapshot(feed)


async def check_model_updates() -> dict[str, object]:
    """Coalesce desktop and browser focus checks through the running host feed."""
    feed = _require_library().update_feed
    await feed.check()
    return model_update_snapshot(feed)


def dismiss_model_update(model_id: str) -> bool:
    """Dismiss only the currently offered version for an exact catalog identity."""
    return _require_library().update_feed.dismiss(model_id)


def disable_model_updates(model_id: str) -> bool:
    """Apply the shared provider-page opt-out from a currently offered catalog identity."""
    return _require_library().update_feed.disable_model(model_id)


async def model_release_history(model_id: str) -> dict[str, object]:
    """Explore the same verified chronology and installed state as the browser family modal."""
    library = _require_library()
    family = await library.update_feed.browse(library.releases, model_id)
    return model_release_history_payload(library.repository, family, model_id)


def queue_model_fingerprint(model_id: str) -> dict[str, object]:
    """Queue one catalog identity using the same worker observed by the browser."""
    library = _require_library()
    return library.queue_fingerprint(model_id).to_payload()


def model_job(job_id: str) -> dict[str, object] | None:
    """Return authoritative progress without starting work or retaining another job state."""
    job = _require_library().jobs.get(job_id)
    return job.to_payload() if job is not None else None


def queue_model_acquisition(request: object) -> dict[str, object]:
    """Validate a desktop recipe through the same provider, installer, and jobs used by the browser."""
    return _require_library().desktop_acquisitions.start(request).to_payload()


def cancel_model_job(job_id: str) -> dict[str, object] | None:
    """Cancel shared work without creating a second frontend-specific job lifecycle."""
    job = _require_library().jobs.cancel(job_id)
    return job.to_payload() if job is not None else None


def active_model_fingerprint(model_id: str) -> dict[str, object] | None:
    """Expose coalescing to desktop callers without reading model bytes."""
    job = _require_library().jobs.active(f"fingerprint-refresh:{model_id}")
    return job.to_payload() if job is not None else None


def prepared_model_asset(asset_id: str) -> dict[str, object] | None:
    """Share an approved image snapshot with sibling HTTP servers without another preview store."""
    asset = _require_library().assets.resolve(asset_id)
    if asset is None:
        return None
    return {
        "assetId": asset.asset_id,
        "path": str(asset.path),
        "contentType": asset.content_type,
        "modifiedNs": str(asset.modified_ns),
        "sizeBytes": asset.size_bytes,
    }


async def refresh_model_catalog(kinds: Sequence[str] | None = None) -> None:
    """Wait for the shared stat-only refresh before returning an explicitly refreshed catalog."""
    library = _require_library()
    job = library.refresh(kinds)
    await library.jobs.wait(job.job_id)
    if job.status != "completed":
        raise RuntimeError(job.error or "The model catalog refresh did not complete.")


async def prepared_civitai_cache_summary() -> dict[str, object]:
    """Read cache usage through the same maintenance authority exposed to HTTP clients."""
    return (await _require_library().civitai_cache.summary()).to_payload()


def queue_civitai_cache_action(action: object) -> dict[str, object]:
    """Queue validated cache maintenance on the running host event loop."""
    return _require_library().cache_actions.start(action).to_payload()


def _require_library() -> ModelLibraryRuntime:
    """Reject calls before host startup instead of building a competing fallback runtime."""
    library = active_model_library()
    if library is None:
        raise RuntimeError("SugarCubes model library is not active yet.")
    return library


def model_library_for_host(host: object) -> ModelLibraryRuntime | None:
    """Reuse registration only for the same running Comfy HTTP application."""
    if _ACTIVE_LIBRARY is not None and _ACTIVE_HOST is not host:
        raise RuntimeError("A model library is already bound to another Comfy host.")
    return _ACTIVE_LIBRARY


def publish_model_library(host: object, library: ModelLibraryRuntime) -> None:
    """Bind exactly one authority after its host registration succeeds."""
    global _ACTIVE_LIBRARY, _ACTIVE_HOST
    current = model_library_for_host(host)
    if current is not None and current is not library:
        raise RuntimeError("Comfy already has a different model library authority.")
    _ACTIVE_LIBRARY, _ACTIVE_HOST = library, host


def release_model_library(library: ModelLibraryRuntime) -> None:
    """Clear only the authority whose host lifecycle has actually stopped."""
    global _ACTIVE_LIBRARY, _ACTIVE_HOST
    if _ACTIVE_LIBRARY is library:
        _ACTIVE_LIBRARY, _ACTIVE_HOST = None, None


sys.modules[MODEL_LIBRARY_HOST_API_MODULE] = sys.modules[__name__]

__all__ = [
    "prepared_model_provider_preferences",
    "configure_model_provider_preferences",
    "prepared_civitai_cache_summary",
    "queue_civitai_cache_action",
    "MODEL_LIBRARY_HOST_API_VERSION",
    "active_model_library",
    "prepared_model_catalog",
    "prepared_model_updates",
    "set_model_update_notifications",
    "check_model_updates",
    "dismiss_model_update",
    "disable_model_updates",
    "model_release_history",
    "queue_model_fingerprint",
    "model_job",
    "queue_model_acquisition",
    "cancel_model_job",
    "active_model_fingerprint",
    "prepared_model_asset",
    "refresh_model_catalog",
    "model_catalog_revision",
]
