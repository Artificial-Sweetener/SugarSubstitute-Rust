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
"""Own secure CivitAI key commands and expose status without returning secret material."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol

from ..domain.credentials import validated_civitai_key


@dataclass(frozen=True, slots=True)
class CredentialStorageStatus:
    """Describe secure persistence availability without exposing a credential location."""

    available: bool
    backend_name: str
    reason: str | None = None


@dataclass(frozen=True, slots=True)
class CredentialTestResult:
    """Report the lightweight provider test without echoing entered secrets or payloads."""

    succeeded: bool
    message: str


class CivitaiCredentialStore(Protocol):
    """Keep operating-system encryption and file lifecycle behind a typed application port."""

    def status(self) -> CredentialStorageStatus:
        """Describe whether secure persistence is available."""

    def load_api_key(self) -> str | None:
        """Read the encrypted credential without retaining plaintext persistence."""

    def save_api_key(self, api_key: str) -> None:
        """Persist a validated key through the operating-system store."""

    def clear_api_key(self) -> None:
        """Remove the stored key without changing host environment configuration."""


class CivitaiCredentialValidator(Protocol):
    """Validate one candidate using the shared provider request budget."""

    async def test(self, api_key: str) -> CredentialTestResult:
        """Perform a lightweight authenticated lookup without writing credentials."""


class CivitaiCredentialService:
    """Keep saving, clearing, status, and testing independent of dialogs and HTTP routes."""

    def __init__(
        self,
        store: CivitaiCredentialStore,
        source: Callable[[], str | None],
        validator: CivitaiCredentialValidator,
    ) -> None:
        """Bind secure persistence, the authoritative precedence source, and provider validation."""
        self._store = store
        self._source = source
        self._validator = validator

    async def status(self) -> dict[str, object]:
        """Expose only configured state and secure-storage capability to frontend consumers."""
        storage = self._store.status()
        configured = await asyncio.to_thread(self._source) is not None
        return {
            "schemaVersion": 1,
            "provider": "civitai",
            "configured": configured,
            "storageAvailable": storage.available,
            "storageBackend": storage.backend_name,
            "storageReason": storage.reason,
        }

    async def save(self, api_key: str) -> None:
        """Store explicit nonempty material without making a network request or returning the key."""
        key = validated_civitai_key(api_key)
        if not self._store.status().available:
            raise ValueError("Secure CivitAI credential storage is unavailable.")
        await asyncio.to_thread(self._store.save_api_key, key)

    async def clear(self) -> None:
        """Remove persisted material and let the authoritative source resolve any host override."""
        await asyncio.to_thread(self._store.clear_api_key)

    async def test(self, api_key: str | None = None) -> CredentialTestResult:
        """Match Substitute's candidate-or-stored-key test without saving or exposing plaintext."""
        candidate = api_key or await asyncio.to_thread(self._source)
        if not candidate:
            return CredentialTestResult(False, "No CivitAI API key is configured.")
        return await self._validator.test(validated_civitai_key(candidate))
