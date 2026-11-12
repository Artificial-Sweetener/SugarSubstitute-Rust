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
"""Safeguard credential precedence, validation, and task-local desktop overrides."""

from __future__ import annotations

import asyncio

import pytest

from sugarcubes.model_library.infrastructure.provider_credentials import (
    civitai_api_key,
    civitai_authorization,
    model_request_credentials,
)


def test_environment_and_desktop_credential_precedence(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Preserve the existing environment contract and restore it after scoped acquisition."""
    monkeypatch.setenv("CIVITAI_API_KEY", "general-fixture")
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)
    assert civitai_api_key() == "general-fixture"
    monkeypatch.setenv("SUGARCUBES_CIVITAI_API_KEY", "extension-fixture")
    assert civitai_api_key() == "extension-fixture"
    with model_request_credentials("desktop-fixture"):
        assert civitai_api_key() == "desktop-fixture"
        with model_request_credentials(None):
            assert civitai_api_key() == "extension-fixture"
        assert civitai_api_key() == "desktop-fixture"
    assert civitai_api_key() == "extension-fixture"


@pytest.mark.parametrize("key", ["x\rheader", "x\nheader", "x" * 4097])
def test_invalid_credentials_never_enter_headers_or_scope(key: str) -> None:
    """Reject header injection and unbounded material before creating a request."""
    with pytest.raises(ValueError, match="invalid"):
        civitai_authorization(lambda: key)
    with pytest.raises(ValueError, match="invalid"):
        with model_request_credentials(key):
            pytest.fail("Invalid credential entered the scoped context.")


def test_desktop_credentials_do_not_cross_async_tasks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep simultaneous desktop acquisitions isolated from one another and their host."""
    monkeypatch.setenv("SUGARCUBES_CIVITAI_API_KEY", "host-fixture")

    async def observe(key: str) -> str | None:
        """Yield inside the scope to expose accidental global credential state."""
        with model_request_credentials(key):
            await asyncio.sleep(0)
            return civitai_api_key()

    async def exercise() -> list[str | None]:
        """Run two credentials concurrently without mutating their inherited context."""
        return list(await asyncio.gather(observe("first"), observe("second")))

    assert asyncio.run(exercise()) == ["first", "second"]
    assert civitai_api_key() == "host-fixture"


def test_saved_credentials_are_the_last_fallback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Use persisted credentials only when desktop and environment sources are absent."""
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    calls: list[bool] = []

    def saved_key() -> str:
        """Record secure-store reads to prove explicit overrides avoid decryption."""
        calls.append(True)
        return "saved-fixture"

    assert civitai_api_key(saved_key) == "saved-fixture"
    calls.clear()
    monkeypatch.setenv("CIVITAI_API_KEY", "environment-fixture")
    assert civitai_api_key(saved_key) == "environment-fixture"
    with model_request_credentials("desktop-fixture"):
        assert civitai_api_key(saved_key) == "desktop-fixture"
    assert not calls
