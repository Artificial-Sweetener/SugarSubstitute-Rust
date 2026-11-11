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
"""Coordinate targeted update checks and shared proposals through one eligibility policy."""

from __future__ import annotations

import asyncio

from .model_update_service import ModelUpdateService
from .release_service import ModelReleaseFamily, ModelReleaseService
from ..domain.model_updates import ModelUpdateProposal


class ModelUpdateFeed:
    """Share one provider check and proposal snapshot across every connected frontend."""

    def __init__(self, service: ModelUpdateService) -> None:
        """Bind the durable policy owner and initialize transient presentation state."""
        self.service = service
        self.proposals: tuple[ModelUpdateProposal, ...] = ()
        self._generation = 0
        self._pending: asyncio.Task[tuple[ModelUpdateProposal, ...]] | None = None
        self._pending_generation = 0

    async def check(self) -> None:
        """Coalesce explicit sweeps and discard results invalidated by a concurrent user decision."""
        if self._pending is None:
            self._pending = asyncio.create_task(self.service.check_updates())
            self._pending_generation = self._generation
        pending = self._pending
        generation = self._pending_generation
        try:
            proposals = await asyncio.shield(pending)
            if generation == self._generation:
                self.proposals = proposals
        finally:
            if pending.done() and self._pending is pending:
                self._pending = None

    def set_enabled(self, enabled: bool) -> None:
        """Persist consent and invalidate in-flight proposals before publication."""
        self.service.set_enabled(enabled)
        self._generation += 1
        self.proposals = ()

    async def check_model(self, location_id: str, *, refresh: bool = False) -> None:
        """Merge a model-specific check without replacing unrelated successful proposals."""
        generation = self._generation
        proposals = await self.service.check_model(location_id, refresh=refresh)
        if generation == self._generation:
            self.proposals = (
                tuple(
                    item
                    for item in self.proposals
                    if item.current.location_id != location_id
                )
                + proposals
            )

    async def browse(
        self, releases: ModelReleaseService, location_id: str
    ) -> ModelReleaseFamily:
        """Refresh deliberate version browsing and publish its resulting update through the same policy."""
        generation = self._generation
        family = await releases.family(location_id, refresh=True)
        proposals = self.service.observe_family(family)
        if generation == self._generation:
            self.proposals = (
                tuple(
                    item
                    for item in self.proposals
                    if item.current.location_id != location_id
                )
                + proposals
            )
        return family

    def dismiss(self, location_id: str) -> bool:
        """Dismiss only the exact candidate offered for this catalog location."""
        proposal = self._proposal(location_id)
        if proposal is None:
            return False
        changed = self.service.dismiss(
            proposal.current.sha256, proposal.candidate.version.version_id
        )
        self._generation += 1
        self.proposals = tuple(
            item
            for item in self.proposals
            if item.current.sha256 != proposal.current.sha256
        )
        return changed

    def disable_model(self, location_id: str) -> bool:
        """Apply the reference's provider-page opt-out to all currently offered versions."""
        proposal = self._proposal(location_id)
        if proposal is None:
            return False
        changed = self.service.disable_model(proposal.current.model_id)
        self._generation += 1
        self.proposals = tuple(
            item
            for item in self.proposals
            if item.current.model_id != proposal.current.model_id
        )
        return changed

    async def close(self) -> None:
        """Release the owned provider task on Comfy host shutdown."""
        if self._pending is not None:
            self._pending.cancel()
            await asyncio.gather(self._pending, return_exceptions=True)
            self._pending = None

    def _proposal(self, location_id: str) -> ModelUpdateProposal | None:
        """Resolve only the exact known offer rather than trusting browser provider identities."""
        return next(
            (
                item
                for item in self.proposals
                if item.current.location_id == location_id
            ),
            None,
        )
