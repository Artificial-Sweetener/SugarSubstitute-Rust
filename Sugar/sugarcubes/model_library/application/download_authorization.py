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
"""Enforce provider access before any entry point starts a remote artifact transfer."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol

from ..domain.provider_releases import ModelArtifactOffer, ModelDownloadAccess


class ModelDownloadAccessProvider(Protocol):
    """Resolve current access for the exact reviewed artifact at its external boundary."""

    async def access_for(self, offer: ModelArtifactOffer) -> ModelDownloadAccess:
        """Return fresh provider evidence rather than trusting retained selection state."""


@dataclass(frozen=True, slots=True)
class ModelDownloadAuthorization:
    """Keep authentication policy authoritative across discovery, versions and workflow recovery."""

    provider: ModelDownloadAccessProvider
    credentials: Callable[[], str | None]

    async def require(self, offer: ModelArtifactOffer) -> None:
        """Reject unknown access or missing credentials before opening a transfer or partial file."""
        access = await self.provider.access_for(offer)
        if access is ModelDownloadAccess.UNKNOWN:
            raise ValueError(
                "Download access could not be verified. Refresh model versions and try again."
            )
        if access is ModelDownloadAccess.API_KEY_REQUIRED and not self.credentials():
            raise ValueError(
                "This model requires a CivitAI API key. Add one in model provider settings."
            )
