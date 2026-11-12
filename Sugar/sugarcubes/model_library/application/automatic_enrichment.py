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
"""Own automatic provider eligibility and durable retry decisions independently of scheduling."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from dataclasses import replace
import time
from typing import Protocol

from .enrichment_service import ModelEnrichmentService
from .enrichment_freshness import ModelEnrichmentFreshness
from .blocking_model_work import complete_blocking_model_work
from .ports import ModelCatalogRepository
from ..domain.models import ModelCatalogEntry, ModelEvidence
from ..domain.enrichment_freshness import ModelEnrichmentRecord


class EnrichmentAttempts(Protocol):
    """Reserve file evidence and retain provider retry deadlines across host restarts."""

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


class AutomaticModelEnrichment:
    """Share the explicit enrichment owner while keeping automatic retries non-destructive."""

    def __init__(
        self,
        repository: ModelCatalogRepository,
        enrichment: ModelEnrichmentService,
        attempts: EnrichmentAttempts,
        freshness: ModelEnrichmentFreshness,
        clock: Callable[[], float] = time.time,
        *,
        enabled: Callable[[str], bool] = lambda _kind: True,
    ) -> None:
        """Bind typed catalog, provider orchestration, durable attempts and a testable clock."""
        self._repository = repository
        self._enrichment = enrichment
        self._attempts = attempts
        self._freshness = freshness
        self._clock = clock
        self._enabled = enabled

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Expose the same freshness decision to foreground work and the background scheduler."""
        return self._enabled(entry.evidence.kind) and self._freshness.needs_refresh(
            entry
        )

    def supports_provider(self, entry: ModelCatalogEntry, provider: str) -> bool:
        """Restrict deliberate refresh scheduling to providers enabled for the model kind."""
        return self._enabled(
            entry.evidence.kind
        ) and provider in self._freshness.providers(entry.evidence.kind)

    async def refresh(
        self, model_id: str, *, automatic: bool = True
    ) -> dict[str, object]:
        """Enrich current evidence once, preserve local previews and back off missing or failed providers."""
        async with self._enrichment.activity.work():
            return await self._refresh(model_id, automatic=automatic)

    async def _refresh(self, model_id: str, *, automatic: bool) -> dict[str, object]:
        """Keep catalog, freshness and retry publication inside one maintenance lease."""
        entry = await complete_blocking_model_work(
            self._repository.find_entry, model_id
        )
        if entry is None:
            raise ValueError("The requested model was not found in the catalog.")
        if not self._enabled(entry.evidence.kind):
            return {"modelId": model_id, "status": "disabled"}
        cached: dict[str, object] = {"modelId": model_id, "status": "cached"}
        policy = self._freshness.policy(entry.evidence.kind)
        provider_scope = self._freshness.provider_scope(entry.evidence.kind)
        providers = self._freshness.providers(entry.evidence.kind)
        retry_policy = f"{provider_scope}|{policy}"
        if automatic and (
            not await complete_blocking_model_work(self.needs_refresh, entry)
            or not await complete_blocking_model_work(
                self._attempts.reserve,
                entry.evidence,
                self._clock(),
                policy=retry_policy,
                providers=providers,
            )
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
            if not self._enabled(current.evidence.kind):
                await complete_blocking_model_work(
                    self._attempts.finish,
                    entry.evidence,
                    None,
                    "disabled",
                    0,
                    policy=retry_policy,
                    providers=providers,
                )
                return {"modelId": model_id, "status": "disabled"}
            retry_at = await complete_blocking_model_work(
                self._attempts.content_retry_at, sha256, policy=retry_policy
            )
            if automatic and (
                not await complete_blocking_model_work(self.needs_refresh, current)
                or retry_at > self._clock()
            ):
                await complete_blocking_model_work(
                    self._attempts.finish,
                    entry.evidence,
                    None,
                    "cached",
                    retry_at,
                    policy=retry_policy,
                    providers=providers,
                )
                return cached
            preserve = automatic and await complete_blocking_model_work(
                self._freshness.preserve_previews, current, policy
            )
            local_preview = preserve and await complete_blocking_model_work(
                self._freshness.local_preview, current
            )
            result = await self._enrichment.refresh(
                replace(current, sha256=sha256), preserve_previews=preserve
            )
            retry_at = self._clock() + 300 if result.warning is not None else 0
            if result.warning is None:
                await complete_blocking_model_work(
                    self._freshness.record,
                    entry.evidence,
                    ModelEnrichmentRecord(
                        sha256,
                        policy,
                        result.metadata,
                        provider_scope,
                        local_preview,
                        result.thumbnail_origin,
                        providers,
                    ),
                )
            await complete_blocking_model_work(
                self._attempts.finish,
                entry.evidence,
                sha256,
                result.status,
                retry_at,
                policy=retry_policy,
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
                policy=retry_policy,
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
                policy=retry_policy,
                providers=providers,
            )
            raise
