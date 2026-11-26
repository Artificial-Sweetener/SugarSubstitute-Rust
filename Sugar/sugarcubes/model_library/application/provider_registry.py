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
"""Select registered metadata providers without coupling enrichment to remote APIs."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Protocol

from ..domain.provider_models import ProviderModelVersion


class ModelHashProvider(Protocol):
    """Resolve one verified content identity through an external provider adapter."""

    async def lookup(
        self,
        sha256: str,
        weights_hash: Callable[[], Awaitable[str | None]],
        *,
        ask_again: bool = False,
    ) -> ProviderModelVersion | None:
        """Return normalized metadata or a confirmed missing result.

        ``weights_hash`` is computed only by a provider that can use it after the full hash
        missed. ``ask_again`` is set for a lookup a person asked for: a provider that remembers
        its recent misses asks anyway.
        """


@dataclass(frozen=True, slots=True)
class RegisteredModelProvider:
    """Declare the model kinds owned by one provider; empty kinds match every kind."""

    kinds: frozenset[str]
    provider: ModelHashProvider


@dataclass(frozen=True, slots=True)
class ModelMetadataProviders:
    """Retain every exact-hash association while choosing labels by declared precedence."""

    registrations: tuple[RegisteredModelProvider, ...]

    async def lookup(
        self,
        sha256: str,
        kind: str,
        weights_hash: Callable[[], Awaitable[str | None]],
        *,
        ask_again: bool = False,
    ) -> tuple[ProviderModelVersion, ...]:
        """Collect supported matches without conflating provider evidence with a filename guess."""
        matches: dict[str, ProviderModelVersion] = {}
        for registration in self.registrations:
            if registration.kinds and kind not in registration.kinds:
                continue
            version = await registration.provider.lookup(
                sha256, weights_hash, ask_again=ask_again
            )
            if version is not None:
                matches.setdefault(version.provider.casefold(), version)
        return tuple(matches.values())
