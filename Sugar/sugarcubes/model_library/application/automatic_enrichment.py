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
"""Own what a model is still owed: a first provider lookup, and the image its offer allows."""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping, Sequence
from dataclasses import replace
import logging
import time
from typing import Protocol

from .enrichment_service import ModelEnrichmentService
from .enrichment_freshness import ModelEnrichmentFreshness
from .legacy_card_repair import LegacyCardRepair
from .blocking_model_work import complete_blocking_model_work
from .ports import ModelCatalogRepository
from .thumbnail_fill import ModelPreviewUnavailableError
from ..domain.card_image import CardImage, card_image
from ..domain.models import ModelCatalogEntry, ModelEvidence
from ..domain.enrichment_freshness import LEARNED_POLICY, ModelEnrichmentRecord
from ..domain.thumbnail_presentation import present_thumbnail

_LOGGER = logging.getLogger(__name__)
# A thumbnail that could not be fetched is tried again by itself, further apart each time.
THUMBNAIL_RETRY_DELAYS = (60.0, 600.0, 3600.0, 86400.0)


class EnrichmentAttempts(Protocol):
    """Reserve file evidence and retain retry deadlines across host restarts."""

    def reserve(
        self,
        evidence: ModelEvidence,
        now: float,
        *,
        policy: str,
        providers: tuple[str, ...],
    ) -> bool:
        """Claim unchanged evidence once until its bounded lease or retry deadline expires."""

    def content_retry_at(self, sha256: str, *, policy: str) -> float:
        """Return the prior provider deadline for identical content at another location."""

    def finish(
        self,
        evidence: ModelEvidence,
        sha256: str | None,
        status: str,
        retry_at: float,
        *,
        policy: str,
        providers: tuple[str, ...],
    ) -> None:
        """Persist one completed decision without trusting client-supplied model identities."""

    def failed_lookups(self) -> frozenset[str]:
        """Name the models whose last lookup failed and is waiting to be tried again."""

    def thumbnail_failures(self) -> Mapping[str, tuple[int, float]]:
        """Return, per model, how many thumbnail fetches failed and when the next is due."""

    def thumbnail_failed(self, model_id: str, failures: int, retry_at: float) -> None:
        """Remember a failed thumbnail fetch and the moment it may be tried again."""

    def thumbnail_fetched(self, model_id: str) -> None:
        """Forget a model's thumbnail failures once its image arrived."""


class AutomaticModelEnrichment:
    """Decide and carry out what each model is owed, never removing what it already has."""

    def __init__(
        self,
        repository: ModelCatalogRepository,
        enrichment: ModelEnrichmentService,
        attempts: EnrichmentAttempts,
        freshness: ModelEnrichmentFreshness,
        clock: Callable[[], float] = time.time,
        *,
        enabled: Callable[[str], bool] = lambda _kind: True,
        repair: LegacyCardRepair | None = None,
    ) -> None:
        """Bind typed catalog, provider orchestration, durable attempts and a testable clock."""
        self._repair = repair
        self._repository = repository
        self._enrichment = enrichment
        self._attempts = attempts
        self._freshness = freshness
        self._clock = clock
        self._enabled = enabled

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Report a file that an enabled provider has never been asked about."""
        return self._enabled(entry.evidence.kind) and self._freshness.needs_refresh(
            entry
        )

    def _lookup_owed(self) -> Callable[[ModelCatalogEntry], bool]:
        """Answer `needs_refresh` for a whole library, reading each kind's setting once.

        Whether a kind is looked up is a stored setting. Reading it once per model made
        building the catalog of a few thousand models take seconds.
        """
        enabled: dict[str, bool] = {}

        def owed(entry: ModelCatalogEntry) -> bool:
            kind = entry.evidence.kind
            if kind not in enabled:
                enabled[kind] = self._enabled(kind)
            return enabled[kind] and self._freshness.needs_refresh(entry)

        return owed

    def owed(
        self, entries: Sequence[ModelCatalogEntry], *, verify_files: bool = True
    ) -> tuple[str, ...]:
        """Name the models with work due now: a first lookup, or an image that can be fetched.

        ``verify_files`` also checks that every stored image is still on disk, which costs a
        stat per file. It is asked for when the library starts and after a folder scan, and
        not by the idle timer, so an idle library does not walk its image folder every minute.
        """
        failures = self._attempts.thumbnail_failures()
        repaired = self._repair.marks.repaired() if self._repair else frozenset()
        policy = self._enrichment.thumbnails.policy()
        now = self._clock()
        lookup_owed = self._lookup_owed()
        due: list[str] = []
        for entry in entries:
            identity = entry.evidence.location_id
            if lookup_owed(entry):
                due.append(identity)
                continue
            shown = present_thumbnail(entry, policy)
            if (
                shown.wanted is not None
                or shown.unblurred
                or (self._repair is not None and self._repair.owed(entry, repaired))
                or (verify_files and self._enrichment.thumbnails.lost(entry))
            ) and failures.get(identity, (0, 0.0))[1] <= now:
                due.append(identity)
        return tuple(due)

    def card_images(self, entries: Sequence[ModelCatalogEntry]) -> dict[str, CardImage]:
        """Say what each model's card shows in place of an image it does not have."""
        failed = self._attempts.failed_lookups()
        failures = self._attempts.thumbnail_failures()
        repaired = self._repair.marks.repaired() if self._repair else frozenset()
        policy = self._enrichment.thumbnails.policy()
        lookup_owed = self._lookup_owed()
        images: dict[str, CardImage] = {}
        for entry in entries:
            identity = entry.evidence.location_id
            lookup = lookup_owed(entry)
            images[identity] = card_image(
                entry,
                present_thumbnail(entry, policy),
                asked=self._freshness.read(entry) is not None,
                lookup_owed=lookup
                or (self._repair is not None and self._repair.owed(entry, repaired)),
                failed=(
                    identity in failed
                    if lookup
                    else failures.get(identity, (0, 0.0))[0] > 0
                ),
            )
        return images

    def supports_provider(self, entry: ModelCatalogEntry, provider: str) -> bool:
        """Restrict deliberate refresh scheduling to providers enabled for the model kind."""
        return self._enabled(
            entry.evidence.kind
        ) and provider in self._freshness.providers(entry.evidence.kind)

    async def refresh(
        self, model_id: str, *, automatic: bool = True
    ) -> dict[str, object]:
        """Give one model what it is owed; a refresh a person asked for always asks again."""
        async with self._enrichment.activity.work():
            return await self._refresh(model_id, automatic=automatic)

    async def _refresh(self, model_id: str, *, automatic: bool) -> dict[str, object]:
        """Keep catalog, freshness and retry publication inside one maintenance lease."""
        entry = await complete_blocking_model_work(
            self._repository.find_entry, model_id
        )
        if entry is None:
            raise ValueError("The requested model was not found in the catalog.")
        cached: dict[str, object] = {"modelId": model_id, "status": "cached"}
        if not self._enabled(entry.evidence.kind):
            await self._fill(model_id)
            return {"modelId": model_id, "status": "disabled"}
        providers = self._freshness.providers(entry.evidence.kind)
        scope = ",".join(providers)
        if automatic and not await complete_blocking_model_work(
            self.needs_refresh, entry
        ):
            await self._fill(model_id)
            return cached
        if automatic and not await complete_blocking_model_work(
            self._attempts.reserve,
            entry.evidence,
            self._clock(),
            policy=scope,
            providers=providers,
        ):
            return cached
        sha256 = entry.sha256
        try:
            sha256 = await self._enrichment.fingerprints.refresh(entry)
            current = await complete_blocking_model_work(
                self._repository.find_entry, model_id
            )
            if current is None or current.evidence != entry.evidence:
                raise ValueError("The model changed before provider enrichment.")
            retry_at = await complete_blocking_model_work(
                self._attempts.content_retry_at, sha256, policy=scope
            )
            if automatic and retry_at > self._clock():
                await complete_blocking_model_work(
                    self._attempts.finish,
                    entry.evidence,
                    None,
                    "cached",
                    retry_at,
                    policy=scope,
                    providers=providers,
                )
                return cached
            identified = replace(current, sha256=sha256)
            result = await self._enrichment.refresh(
                identified, replace_labels=not automatic
            )
            previous = await complete_blocking_model_work(
                self._freshness.read, identified
            )
            await complete_blocking_model_work(
                self._freshness.record,
                entry.evidence,
                ModelEnrichmentRecord(
                    sha256,
                    LEARNED_POLICY,
                    result.metadata,
                    scope,
                    False,
                    result.thumbnail_origin,
                    tuple(
                        dict.fromkeys(
                            (*(previous.providers if previous else ()), *providers)
                        )
                    ),
                ),
            )
            if result.warning is not None:
                await self._thumbnail_failed(model_id)
            elif result.metadata is not None:
                await complete_blocking_model_work(
                    self._attempts.thumbnail_fetched, model_id
                )
            await complete_blocking_model_work(
                self._attempts.finish,
                entry.evidence,
                sha256,
                result.status,
                0,
                policy=scope,
                providers=providers,
            )
            return result.to_payload()
        except asyncio.CancelledError:
            await complete_blocking_model_work(
                self._attempts.finish,
                entry.evidence,
                None,
                "cancelled",
                self._clock() + 60,
                policy=scope,
                providers=providers,
            )
            raise
        except (OSError, RuntimeError, ValueError):
            # The job boundary retains the original failure and stack; persistence only throttles retries.
            await complete_blocking_model_work(
                self._attempts.finish,
                entry.evidence,
                sha256,
                "failed",
                self._clock() + 300,
                policy=scope,
                providers=providers,
            )
            raise

    async def _fill(self, model_id: str) -> None:
        """Fetch an identified model's owed image when its retry is due; never ask a provider what it is."""
        failures = await complete_blocking_model_work(self._attempts.thumbnail_failures)
        if failures.get(model_id, (0, 0.0))[1] > self._clock():
            return
        try:
            changed = await self._repaired(model_id)
            changed = await self._enrichment.thumbnails.fill(model_id) or changed
        except (OSError, ValueError) as error:
            _LOGGER.warning(
                "Model card repair failed; it will be retried. "
                "operation=model_card_repair cause=%s",
                type(error).__name__,
                extra={"operation": "model_card_repair", "model_id": model_id},
            )
            await self._thumbnail_failed(model_id)
            return
        except ModelPreviewUnavailableError as error:
            _LOGGER.warning(
                "Model thumbnail fetch failed; it will be retried. "
                "operation=model_thumbnail_fill cause=%s",
                error.reason,
                extra={
                    "operation": "model_thumbnail_fill",
                    "model_id": model_id,
                    "cause": error.reason,
                },
                exc_info=None if error.expected else error,
            )
            await self._thumbnail_failed(model_id)
            return
        if changed or model_id in failures:
            await complete_blocking_model_work(
                self._attempts.thumbnail_fetched, model_id
            )

    async def _repaired(self, model_id: str) -> bool:
        """Give a card learned before offers were stored its offer, once; report any repair."""
        if self._repair is None:
            return False
        entry = await complete_blocking_model_work(
            self._repository.find_entry, model_id
        )
        if entry is None or not self._repair.owed(
            entry, await complete_blocking_model_work(self._repair.marks.repaired)
        ):
            return False
        await self._repair.repair(entry)
        return True

    async def _thumbnail_failed(self, model_id: str) -> None:
        """Schedule the next try further out each time a model's thumbnail fails again."""
        failures = await complete_blocking_model_work(self._attempts.thumbnail_failures)
        count = failures.get(model_id, (0, 0.0))[0]
        delay = THUMBNAIL_RETRY_DELAYS[min(count, len(THUMBNAIL_RETRY_DELAYS) - 1)]
        await complete_blocking_model_work(
            self._attempts.thumbnail_failed, model_id, count + 1, self._clock() + delay
        )
