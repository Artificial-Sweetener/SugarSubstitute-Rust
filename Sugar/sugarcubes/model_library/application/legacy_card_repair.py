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
"""Give a card learned before provider offers were stored the offer it never recorded."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Protocol

from .blocking_model_work import complete_blocking_model_work
from .ports import ModelCatalogRepository
from ..domain.models import ModelCatalogEntry, PreviewCandidate
from ..domain.provider_metadata import with_offered_previews

_PROVIDER = "civitai"


class RepairedCards(Protocol):
    """Remember which file contents have had their one repair."""

    def repaired(self) -> frozenset[str]:
        """Name every content hash whose provider offer was already asked for."""

    def mark_repaired(self, sha256: str) -> None:
        """Record a completed repair, so the provider is asked for an offer only once."""


@dataclass(frozen=True, slots=True)
class LegacyCardRepair:
    """Ask a provider, by the version a card already names, which images it offers.

    A card learned before offers were stored knows its provider version and at most one
    image. Asking by version cannot change what the model is, and an unknown version leaves
    the card as it is.
    """

    repository: ModelCatalogRepository
    offered: Callable[[str], Awaitable[tuple[PreviewCandidate, ...]]]
    marks: RepairedCards

    def owed(self, entry: ModelCatalogEntry, repaired: frozenset[str]) -> bool:
        """Report a card with a provider version, no stored offer, and no repair yet."""
        metadata = entry.metadata
        return (
            entry.sha256 is not None
            and entry.sha256 not in repaired
            and metadata is not None
            and metadata.provider.casefold() == _PROVIDER
            and bool(metadata.provider_version_id)
            and not metadata.previews
        )

    async def repair(self, entry: ModelCatalogEntry) -> None:
        """Store the offer for a card's version; a provider or network failure propagates."""
        if entry.sha256 is None or entry.metadata is None:
            return
        previews = await self.offered(entry.metadata.provider_version_id)
        records = await complete_blocking_model_work(
            self.repository.provider_records, entry.sha256
        )
        await complete_blocking_model_work(
            self.repository.save_provider_metadata,
            entry.sha256,
            with_offered_previews(records, _PROVIDER, previews),
        )
        await complete_blocking_model_work(self.marks.mark_repaired, entry.sha256)
