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
"""Prove real Windows encryption, shared provider validation, and private credential lifecycle."""

from __future__ import annotations

import asyncio
import base64
import logging
from pathlib import Path
import sys
from types import ModuleType

import aiohttp
import pytest

from sugarcubes.model_library.infrastructure.civitai_credential_store import (
    WindowsCivitaiCredentialStore,
)
from sugarcubes.model_library.infrastructure.comfy_credentials import (
    host_civitai_credential_store,
)
from sugarcubes.model_library.infrastructure.provider_credentials import (
    civitai_api_key,
    model_request_credentials,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


@pytest.mark.skipif(
    sys.platform != "win32", reason="Require current-user Windows DPAPI."
)
def test_real_secure_storage_is_encrypted_restartable_and_atomic(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Store only synthetic keys and preserve prior encrypted material after publication failure."""
    store = WindowsCivitaiCredentialStore(tmp_path)
    secret = "fixture-secret-never-a-real-provider-key"
    assert store.status().available
    store.save_api_key("  " + secret + "  ")
    path = tmp_path / "civitai_api_key.dpapi"
    encrypted = path.read_bytes()
    assert secret.encode() not in encrypted
    assert secret.encode() not in base64.b64decode(encrypted)
    assert WindowsCivitaiCredentialStore(tmp_path).load_api_key() == secret

    def fail_replace(source: object, destination: object) -> None:
        """Exercise atomic publication failure without touching the previous ciphertext."""
        raise OSError("Synthetic file publication failure.")

    with monkeypatch.context() as scope:
        scope.setattr(
            "sugarcubes.model_library.infrastructure.civitai_credential_store.os.replace",
            fail_replace,
        )
        with pytest.raises(OSError):
            store.save_api_key("replacement-fixture")
    assert path.read_bytes() == encrypted
    assert store.load_api_key() == secret
    assert list(tmp_path.iterdir()) == [path]
    store.clear_api_key()
    assert store.load_api_key() is None
    store.clear_api_key()


def test_corrupt_credentials_fail_closed_without_echoing_content(
    tmp_path: Path,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Retain diagnostic context while keeping corrupt material and private paths out of logs."""
    secret = "corrupt-fixture-secret"
    path = tmp_path / "civitai_api_key.dpapi"
    path.write_text(secret, encoding="ascii")
    with caplog.at_level(logging.WARNING):
        assert WindowsCivitaiCredentialStore(tmp_path).load_api_key() is None
    assert "could not be loaded" in caplog.text
    assert secret not in caplog.text and str(tmp_path) not in caplog.text
    path.write_bytes(b"x" * (64 * 1024 + 1))
    assert WindowsCivitaiCredentialStore(tmp_path).load_api_key() is None


def test_host_credentials_use_private_system_directory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolve only the host's protected system-user namespace, never a model root or caller path."""
    module = ModuleType("folder_paths")
    calls: list[str] = []

    def system_root(name: str) -> str:
        """Observe the requested host namespace without reading any actual credentials."""
        calls.append(name)
        return str(tmp_path / "__sugarcubes")

    setattr(module, "get_system_user_directory", system_root)
    monkeypatch.setitem(sys.modules, "folder_paths", module)
    store = host_civitai_credential_store()
    assert calls == ["sugarcubes"]
    if sys.platform == "win32":
        store.save_api_key("synthetic-private-key")
        assert (tmp_path / "__sugarcubes/credentials/civitai_api_key.dpapi").exists()
    setattr(module, "get_system_user_directory", lambda name: "relative")
    with pytest.raises(RuntimeError, match="invalid"):
        host_civitai_credential_store()
    delattr(module, "get_system_user_directory")
    with pytest.raises(RuntimeError, match="unavailable"):
        host_civitai_credential_store()


@pytest.mark.skipif(
    sys.platform != "win32", reason="Require current-user Windows DPAPI."
)
@pytest.mark.parametrize("http_status", [200, 404, 401, 403, 429, 503])
def test_runtime_credentials_share_transport_and_restore_key_context(
    http_status: int,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Test a candidate without replacing the saved key, leaking secrets, or bypassing cooldown."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)
    store = WindowsCivitaiCredentialStore(tmp_path / "private")
    session = TransportSession(
        TransportResponse(
            http_status,
            b'{"id":1,"modelId":2,"name":"fixture","model":{"name":"fixture"}}',
        )
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path / "models"), credential_store=store
    )

    async def exercise() -> None:
        """Drive real command and transport owners against only an external response fixture."""
        assert not (await runtime.credentials.status())["configured"]
        assert not (await runtime.credentials.test()).succeeded
        await runtime.credentials.save("stored-fixture")
        result = await runtime.credentials.test("candidate-fixture")
        assert result.succeeded is (http_status in (200, 404))
        assert store.load_api_key() == "stored-fixture"
        assert civitai_api_key(store.load_api_key) == "stored-fixture"
        assert session.headers[0]["Authorization"] == "Bearer candidate-fixture"
        assert session.requests[0][0].endswith("0" * 64)
        with model_request_credentials("desktop-fixture"):
            assert civitai_api_key(store.load_api_key) == "desktop-fixture"
        if http_status in (429, 503):
            assert not (await runtime.credentials.test()).succeeded
            assert len(session.requests) == 1
        await runtime.credentials.clear()
        assert not (await runtime.credentials.status())["configured"]
        await runtime.jobs.close()

    asyncio.run(exercise())


@pytest.mark.parametrize("body", [b"{}", b"null", b'{"id":true}'])
def test_invalid_provider_success_is_not_a_valid_credential(
    body: bytes,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Require actual normalized FOUND metadata rather than treating arbitrary HTTP 200 as success."""
    session = TransportSession(TransportResponse(200, body))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    assert not asyncio.run(runtime.credentials.test("candidate-fixture")).succeeded


def test_candidate_connection_failure_redacts_secret_and_restores_override(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Keep a provider exception from leaking credentials or changing the active host override."""
    secret = "synthetic-failure-candidate"
    monkeypatch.setenv("SUGARCUBES_CIVITAI_API_KEY", "host-fixture")

    def fail_session(**kwargs: object) -> None:
        """Replace only the external network boundary with a deliberately sensitive exception."""
        raise aiohttp.ClientConnectionError(secret)

    monkeypatch.setattr(aiohttp, "ClientSession", fail_session)
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    with caplog.at_level(logging.WARNING):
        result = asyncio.run(runtime.credentials.test(secret))
    assert not result.succeeded and secret not in result.message
    assert secret not in caplog.text
    assert "ClientConnectionError" in caplog.text
    assert civitai_api_key() == "host-fixture"
