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
"""Prove sibling extensions share one host-bound model catalog and job lifecycle."""

from __future__ import annotations

from tests.model_library.support.enrichment import ObservedProvider

import asyncio
import hashlib
from io import BytesIO
import sys
from pathlib import Path

from aiohttp import web
import pytest
from PIL import Image

from sugarcubes.model_library import host_api
from sugarcubes.model_library.infrastructure.comfy_roots import ComfyModelRoots
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.integration import install_model_library
from sugarcubes.model_library.infrastructure.civitai_credential_store import (
    WindowsCivitaiCredentialStore,
)
from sugarcubes_model_roots import ModelRootConfiguration
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.comfy_queue import AcceptedPromptQueue


class FixtureModelHost:
    """Expose only the Comfy route registry and HTTP lifecycle boundary."""

    def __init__(self) -> None:
        """Keep a real application separate from unrelated extension fixtures."""
        self.routes = web.RouteTableDef()
        self.app = web.Application()
        self.prompt_queue = AcceptedPromptQueue()


def test_host_registration_reuses_one_authority_and_releases_it(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Register routes once, reject another active host, and close every owned job."""
    roots = FixtureFileRoots(tmp_path)
    monkeypatch.setattr(ComfyModelRoots, "from_host", lambda: roots)
    monkeypatch.setattr(ModelRootConfiguration, "from_host", lambda: object())
    monkeypatch.setattr(
        "sugarcubes.model_library.integration.host_civitai_credential_store",
        lambda: WindowsCivitaiCredentialStore(tmp_path / "private"),
    )
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", None)
    monkeypatch.setattr(host_api, "_ACTIVE_HOST", None)
    host = FixtureModelHost()
    runtime = install_model_library(host)
    route_count = len(host.routes)
    cleanup_count = len(host.app.on_cleanup)
    assert install_model_library(host) is runtime
    assert len(host.routes) == route_count and len(host.app.on_cleanup) == cleanup_count
    assert host_api.active_model_library() is runtime
    assert sys.modules[host_api.MODEL_LIBRARY_HOST_API_MODULE] is host_api
    with pytest.raises(RuntimeError, match="another Comfy host"):
        install_model_library(FixtureModelHost())

    async def close() -> None:
        """Drive the real host shutdown signal instead of manually resetting runtime state."""
        host.app.freeze()
        await host.app.startup()
        initial = runtime.start_initial_scan()
        assert runtime.start_initial_scan() is initial
        await runtime.jobs.wait(initial.job_id)
        assert initial.status == "completed"
        assert host_api.prepared_model_catalog()["schemaVersion"] == 1
        await host.app.cleanup()

    asyncio.run(close())
    assert host_api.active_model_library() is None


def test_authority_replacement_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep competing worker pools from silently replacing the running public authority."""
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", None)
    monkeypatch.setattr(host_api, "_ACTIVE_HOST", None)
    host = object()
    library = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    host_api.publish_model_library(host, library)
    with pytest.raises(RuntimeError, match="different model library authority"):
        host_api.publish_model_library(
            host,
            ModelLibraryRuntime(
                FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
            ),
        )
    assert host_api.active_model_library() is library


def test_desktop_and_browser_share_explicit_fingerprint_execution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Observe one real tiny-file job from both public consumers without provider requests."""
    content = b"shared identity fixture, never real model weights"
    (tmp_path / "fixture.safetensors").write_bytes(content)
    library = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", library)
    monkeypatch.setattr(host_api, "_ACTIVE_HOST", object())
    library.catalog.refresh()
    identity = library.repository.list_entries()[0].evidence.location_id

    async def exercise() -> None:
        """Drive the shared event loop and verify observable coalescing and committed storage."""
        desktop = host_api.queue_model_fingerprint(identity)
        browser = await library.refresh_fingerprint(identity)
        assert desktop["jobId"] == browser.job_id
        active = host_api.active_model_fingerprint(identity)
        assert active is not None and active["jobId"] == browser.job_id
        await library.jobs.wait(browser.job_id)
        observed = host_api.model_job(browser.job_id)
        assert observed is not None and observed["status"] == "completed"
        assert browser.result["sha256"] == hashlib.sha256(content).hexdigest()
        assert host_api.active_model_fingerprint(identity) is None
        assert host_api.model_job("unknown") is None
        assert library.repository.list_entries()[0].sha256 == browser.result["sha256"]
        await library.automatic_enrichment.close()
        await library.jobs.close()

    asyncio.run(exercise())


def test_shared_preview_resolution_uses_one_immutable_asset_owner(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Serve identical prepared bytes to sibling routes and reject arbitrary paths."""
    library = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", library)
    content = BytesIO()
    Image.new("RGB", (8, 4), "blue").save(content, format="WEBP")
    identity = library.assets.publish(content.getvalue())
    payload = host_api.prepared_model_asset(identity)
    assert payload is not None
    assert payload["assetId"] == identity and payload["contentType"] == "image/webp"
    assert isinstance(payload["path"], str)
    assert Path(payload["path"]).read_bytes() == content.getvalue()
    assert payload["sizeBytes"] == len(content.getvalue())
    assert host_api.prepared_model_asset("f" * 64) is None
    with pytest.raises(ValueError):
        host_api.prepared_model_asset("../private.png")


def test_desktop_explicit_refresh_waits_for_shared_committed_discovery(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Expose a newly added tiny fixture after refresh without inline hashing or provider work."""
    library = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", library)
    assert library.repository.list_entries() == ()
    (tmp_path / "new.safetensors").write_bytes(
        b"new catalog fixture, never model weights"
    )

    async def exercise() -> None:
        """Observe committed evidence before the desktop caller reads its response."""
        await host_api.refresh_model_catalog(("loras",))
        entries = library.repository.list_entries()
        assert len(entries) == 1 and entries[0].evidence.value == "new.safetensors"
        assert entries[0].sha256 is None
        await library.automatic_enrichment.close()
        await library.jobs.close()

    asyncio.run(exercise())
