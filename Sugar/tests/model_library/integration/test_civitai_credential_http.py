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
"""Prove credential HTTP commands never expose secret material or weaken secure persistence."""

from __future__ import annotations

import asyncio
from pathlib import Path
import sys

import aiohttp
from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library.credential_api import register_model_credential_routes
from sugarcubes.model_library.infrastructure.civitai_credential_store import (
    WindowsCivitaiCredentialStore,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)

_PATH = "/sugarcubes/v1/models/credentials/civitai"


@pytest.mark.skipif(
    sys.platform != "win32", reason="Require current-user Windows DPAPI."
)
def test_http_secure_status_save_test_clear_and_invalid_requests(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Cross real HTTP, command, encrypted-file, and provider boundaries using synthetic keys only."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)
    session = TransportSession(TransportResponse(404, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Keep all synthetic persistence in the test's private directory and close its jobs."""
        store = WindowsCivitaiCredentialStore(tmp_path / "private")
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path / "models"), credential_store=store
        )
        routes = web.RouteTableDef()
        register_model_credential_routes(routes, runtime.credentials)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get(_PATH)
            assert response.status == 200
            assert not (await response.json())["configured"]
            assert response.headers["Cache-Control"] == "no-store"
            secret = "http-synthetic-secret"
            response = await client.put(_PATH, json={"apiKey": "  " + secret + "  "})
            assert response.status == 200
            assert (await response.json())["configured"]
            assert secret not in await response.text()
            assert str(tmp_path) not in await response.text()
            assert not session.requests
            response = await client.post(
                _PATH + "/test", json={"apiKey": "candidate-secret"}
            )
            assert response.status == 200
            assert (await response.json())["succeeded"]
            assert "candidate-secret" not in await response.text()
            assert session.headers[-1]["Authorization"] == "Bearer candidate-secret"
            assert store.load_api_key() == secret
            response = await client.post(_PATH + "/test", json={})
            assert (await response.json())["succeeded"]
            assert session.headers[-1]["Authorization"] == "Bearer " + secret
            for payload in (
                {"apiKey": ""},
                {"apiKey": 42},
                {"apiKey": "\nsecret"},
                {"apiKey": secret, "path": "../public"},
                {"apiKey": "x" * 4097},
            ):
                response = await client.put(_PATH, json=payload)
                assert response.status == 400
                assert secret not in await response.text()
                assert store.load_api_key() == secret
            response = await client.put(_PATH, data=b"x" * (16 * 1024 + 1))
            assert response.status == 400
            response = await client.put(_PATH, data=b"\xff")
            assert response.status == 400
            response = await client.delete(_PATH)
            assert response.status == 200
            assert not (await response.json())["configured"]
            assert store.load_api_key() is None
        await runtime.jobs.close()

    asyncio.run(exercise())


@pytest.mark.skipif(
    sys.platform != "win32", reason="Require current-user Windows DPAPI."
)
def test_http_file_failures_do_not_echo_secret_or_claim_success(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Propagate failed save/removal as generic command errors while preserving stored material."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)

    async def exercise() -> None:
        """Drive failed atomic publication and removal through the real HTTP boundary."""
        store = WindowsCivitaiCredentialStore(tmp_path / "private")
        store.save_api_key("original-fixture")
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path / "models"), credential_store=store
        )
        routes = web.RouteTableDef()
        register_model_credential_routes(routes, runtime.credentials)
        app = web.Application()
        app.add_routes(routes)

        def fail_command(*arguments: object) -> None:
            """Include a synthetic secret in the exception to prove HTTP/log redaction."""
            raise OSError("candidate-secret")

        async with TestClient(TestServer(app)) as client:
            with monkeypatch.context() as scope:
                scope.setattr(store, "save_api_key", fail_command)
                response = await client.put(_PATH, json={"apiKey": "candidate-secret"})
                assert response.status == 500
                assert "candidate-secret" not in await response.text()
                assert store.load_api_key() == "original-fixture"
                scope.setattr(store, "clear_api_key", fail_command)
                response = await client.delete(_PATH)
                assert response.status == 500
                assert store.load_api_key() == "original-fixture"
            monkeypatch.setenv("SUGARCUBES_CIVITAI_API_KEY", "environment-fixture")
            response = await client.delete(_PATH)
            assert response.status == 200
            assert (await response.json())["configured"]
            assert store.load_api_key() is None
        await runtime.jobs.close()

    asyncio.run(exercise())


def test_http_unavailable_store_never_saves_plaintext(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Expose unavailable secure storage without redirecting secrets into model asset folders."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)

    async def exercise() -> None:
        """Run the host-neutral composition, which intentionally has no persistence adapter."""
        runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
        routes = web.RouteTableDef()
        register_model_credential_routes(routes, runtime.credentials)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get(_PATH)
            assert not (await response.json())["storageAvailable"]
            response = await client.put(_PATH, json={"apiKey": "synthetic-secret"})
            assert response.status == 400
            assert "synthetic-secret" not in await response.text()
            assert not (await runtime.credentials.status())["configured"]
        assert not list(tmp_path.rglob("*.dpapi"))
        await runtime.jobs.close()

    asyncio.run(exercise())
