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
"""Own Substitute's update consent, recent usage, dismissal and proposal lifecycle."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace
from datetime import UTC, datetime, timedelta

from .ports import ModelCatalogRepository
from .release_service import ModelReleaseService, ModelReleaseFamily
from .usage_repository import ModelUsageRepository
from ..domain.models import ModelCatalogEntry
from ..domain.provider_releases import ModelProviderRelease
from ..domain.model_updates import (
    ModelUpdateProposal,
    ModelUpdateState,
    ModelUsageRecord,
)

_UPDATE_KINDS = frozenset({"checkpoints", "diffusion_models", "loras"})


class ModelUpdateService:
    """Port update policy while delegating compatibility to the authoritative release service."""

    def __init__(
        self,
        usage: ModelUsageRepository,
        catalog: ModelCatalogRepository,
        releases: ModelReleaseService,
        *,
        clock: Callable[[], datetime] = lambda: datetime.now(UTC),
        relevance_days: int = 90,
    ) -> None:
        """Bind durable state and exact installed evidence without owning network or persistence mechanics."""
        if relevance_days < 1:
            raise ValueError("Model update relevance window must be positive.")
        self.usage = usage
        self.catalog = catalog
        self.releases = releases
        self.clock = clock
        self.relevance_days = relevance_days

    def set_enabled(self, enabled: bool) -> None:
        """Persist a deliberate notification preference independently of picker activity."""
        self.usage.change(lambda state: replace(state, enabled=enabled))

    def record_usage(self, entry: ModelCatalogEntry) -> ModelUsageRecord | None:
        """Count one successfully dispatched exact artifact without reading weights."""
        return self._record(entry, dispatched=True)

    def _record(
        self, entry: ModelCatalogEntry, *, dispatched: bool
    ) -> ModelUsageRecord | None:
        """Retain observed provider identity while counting only actual workflow dispatches."""
        if entry.sha256 is None:
            return None
        now = self.clock()
        if now.tzinfo is None:
            raise ValueError("Model usage timestamps must be timezone-aware.")
        metadata = entry.metadata

        def transition(state: ModelUpdateState) -> ModelUpdateState:
            """Preserve opt-out and missing provider evidence when another dispatch is recorded."""
            previous = next(
                (record for record in state.records if record.sha256 == entry.sha256),
                None,
            )
            model_id = (
                metadata.provider_model_id
                if metadata and metadata.provider == "civitai"
                else ""
            )
            model_id = model_id or (previous.model_id if previous else "")
            record = ModelUsageRecord(
                entry.sha256 or "",
                entry.evidence.location_id,
                entry.evidence.kind,
                model_id,
                (
                    metadata.provider_version_id
                    if metadata and metadata.provider == "civitai"
                    else ""
                )
                or (previous.version_id if previous else ""),
                (metadata.base_model if metadata else "")
                or (previous.base_model if previous else ""),
                (previous.usage_count if previous else 0) + int(dispatched),
                (
                    now.astimezone(UTC)
                    if dispatched
                    else (previous.last_used_at if previous else None)
                ),
                previous.dismissed_version_id if previous else "",
                bool(model_id)
                and any(
                    record.model_id == model_id and record.updates_disabled_for_model
                    for record in state.records
                ),
            )
            return replace(
                state,
                records=(
                    tuple(
                        record if item.sha256 == record.sha256 else item
                        for item in state.records
                    )
                    if previous
                    else (*state.records, record)
                ),
            )

        state = self.usage.change(transition)
        return next(record for record in state.records if record.sha256 == entry.sha256)

    async def check_model(
        self, location_id: str, *, refresh: bool = False
    ) -> tuple[ModelUpdateProposal, ...]:
        """Check a known model on initial pull, daily selection or explicit version browsing."""
        entry = self.catalog.find_entry(location_id)
        if entry is None:
            raise ValueError("The requested model was not found.")
        if entry.evidence.kind not in _UPDATE_KINDS or entry.sha256 is None:
            return ()
        metadata = entry.metadata
        previous = next(
            (
                record
                for record in self.usage.load().records
                if record.sha256 == entry.sha256
            ),
            None,
        )
        if not refresh and (
            (previous and previous.updates_disabled_for_model)
            or any(
                record.updates_disabled_for_model
                and metadata is not None
                and record.model_id == metadata.provider_model_id
                for record in self.usage.load().records
            )
        ):
            return ()
        if (
            metadata is None
            or metadata.provider != "civitai"
            or not metadata.provider_model_id
            or not metadata.provider_version_id
            or not metadata.base_model.strip()
        ):
            return ()
        family = await self.releases.family(location_id, refresh=refresh)
        return self.observe_family(family)

    def observe_family(
        self, family: ModelReleaseFamily
    ) -> tuple[ModelUpdateProposal, ...]:
        """Project successfully checked compatible history without inventing a workflow dispatch."""
        if family.current.evidence.kind not in _UPDATE_KINDS:
            return ()
        record = self._record(family.current, dispatched=False)
        if record is None or not self.usage.load().enabled:
            return ()
        proposal = self._proposal(record, family.releases, self.catalog.list_entries())
        return (proposal,) if proposal else ()

    @staticmethod
    def _proposal(
        record: ModelUsageRecord,
        family: tuple[ModelProviderRelease, ...],
        installed: tuple[ModelCatalogEntry, ...],
    ) -> ModelUpdateProposal | None:
        """Apply dismissal, page opt-out and installed-artifact suppression for every trigger."""
        candidate = family[-1] if family else None
        if (
            record.updates_disabled_for_model
            or candidate is None
            or candidate.version.version_id
            in (record.version_id, record.dismissed_version_id)
        ):
            return None
        if candidate.offers[0].sha256 in {
            item.sha256 for item in installed if item.evidence.kind == record.kind
        }:
            return None
        return ModelUpdateProposal(record, candidate)

    def dismiss(self, sha256: str, version_id: str) -> bool:
        """Hide only the offered version until another candidate appears."""
        if not version_id.isdigit() or int(version_id) <= 0:
            raise ValueError("Dismissed model version must be positive.")
        changed = False

        def transition(state: ModelUpdateState) -> ModelUpdateState:
            """Leave unrelated models and page opt-outs intact."""
            nonlocal changed
            changed = any(record.sha256 == sha256 for record in state.records)
            return replace(
                state,
                records=tuple(
                    (
                        replace(record, dismissed_version_id=version_id)
                        if record.sha256 == sha256
                        else record
                    )
                    for record in state.records
                ),
            )

        self.usage.change(transition)
        return changed

    def disable_model(self, model_id: str) -> bool:
        """Stop checking every recorded version belonging to the chosen provider page."""
        if not model_id.isdigit() or int(model_id) <= 0:
            raise ValueError("Provider model page identifier must be positive.")
        changed = False

        def transition(state: ModelUpdateState) -> ModelUpdateState:
            """Persist the source's page-level opt-out across current and later versions."""
            nonlocal changed
            changed = any(record.model_id == model_id for record in state.records)
            return replace(
                state,
                records=tuple(
                    (
                        replace(record, updates_disabled_for_model=True)
                        if record.model_id == model_id
                        else record
                    )
                    for record in state.records
                ),
            )

        self.usage.change(transition)
        return changed

    async def check_updates(self) -> tuple[ModelUpdateProposal, ...]:
        """Check recent usage when enabled and suppress dismissed or installed candidates."""
        state = self.usage.load()
        if not state.enabled:
            return ()
        now = self.clock()
        if now.tzinfo is None:
            raise ValueError("Model update timestamps must be timezone-aware.")
        cutoff = now.astimezone(UTC) - timedelta(days=self.relevance_days)
        disabled = {
            record.model_id
            for record in state.records
            if record.updates_disabled_for_model
        }
        installed = self.catalog.list_entries()
        proposals: list[ModelUpdateProposal] = []
        for record in sorted(
            state.records,
            key=lambda item: item.last_used_at or datetime.min.replace(tzinfo=UTC),
            reverse=True,
        ):
            if (
                (record.last_used_at is not None and record.last_used_at < cutoff)
                or record.kind not in _UPDATE_KINDS
                or not record.model_id
                or not record.version_id
                or record.model_id in disabled
            ):
                continue
            family = await self.releases.history_for_usage(record)
            proposal = self._proposal(record, family, installed)
            if proposal:
                proposals.append(proposal)
        return tuple(proposals)
