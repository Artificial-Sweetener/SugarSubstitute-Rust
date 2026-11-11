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
"""Prove source-priority merging and exact acquisition against Substitute's engine contract."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

import asyncio
from collections.abc import Callable
from dataclasses import replace

import pytest

from sugarcubes.model_library.application.model_suggestion_engine import (
    ModelSuggestionEngine,
)
from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionReference,
)
from sugarcubes.model_library.domain.provider_releases import (
    InstalledModelArtifact,
    ModelArtifactOffer,
)

CONTEXT = ModelSuggestionContext("upscale_models")


def suggestion(
    provider: str, digest: str, *, context: ModelSuggestionContext = CONTEXT
) -> ModelSuggestion:
    """Build independent provider offers for one exact content identity."""
    artifact = ModelArtifactOffer(
        digest,
        "model.pth",
        digest.lower(),
        7,
        f"https://example.test/{provider}/model.pth",
        context.artifact_kind,
        provider,
    )
    return ModelSuggestion(
        context,
        provider + " title",
        "Version",
        "Creator",
        digest,
        (
            ModelSuggestionOffer(
                ModelSuggestionReference(provider, provider, "model", "version"),
                replace(artifact, access=ModelDownloadAccess.PUBLIC),
                f"https://example.test/{provider}",
                None,
                1,
            ),
        ),
    )


class ProviderFixture:
    """Replace only the external provider boundary while observing exact routing."""

    def __init__(
        self,
        provider_id: str,
        items: tuple[ModelSuggestion, ...],
        *,
        supported: bool = True,
    ) -> None:
        """Retain responses and calls without reproducing engine policy in the fixture."""
        self.provider_id = provider_id
        self.items = items
        self.supported = supported
        self.requests: list[
            tuple[
                ModelSuggestionContext, ModelSuggestionAccessPolicy, int, frozenset[str]
            ]
        ] = []
        self.acquisitions: list[tuple[ModelSuggestion, ModelSuggestionOffer]] = []

    def supports(self, context: ModelSuggestionContext) -> bool:
        """Expose declared support independently of the response payload."""
        return self.supported

    def browse_url(self, context: ModelSuggestionContext) -> str:
        """Provide a source-labelled page without network access."""
        return f"https://example.test/{self.provider_id}"

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str],
    ) -> tuple[ModelSuggestion, ...]:
        """Record policy forwarding and return the entire fixture response."""
        self.requests.append((context, access_policy, limit, excluded_sha256))
        return self.items

    async def acquire(
        self,
        selected: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Record the chosen source and preserve transport progress delegation."""
        self.acquisitions.append((selected, offer))
        if progress is not None:
            progress(7, 7)
        return InstalledModelArtifact(offer.artifact.file_name, offer.artifact)


def test_priority_hash_merging_limit_exclusions_and_context_match_reference() -> None:
    """Keep primary labels while merging later sources even after reaching the card limit."""
    first = ProviderFixture(
        "openmodeldb",
        (
            suggestion("openmodeldb", "a" * 64),
            suggestion("openmodeldb", "B" * 64),
            suggestion("openmodeldb", "a" * 64),
            suggestion(
                "openmodeldb", "c" * 64, context=ModelSuggestionContext("loras")
            ),
        ),
    )
    second = ProviderFixture(
        "civitai", (suggestion("civitai", "A" * 64), suggestion("civitai", "d" * 64))
    )
    unsupported = ProviderFixture("unsupported", (), supported=False)
    engine = ModelSuggestionEngine((first, second, unsupported))
    result = asyncio.run(
        engine.suggest(
            CONTEXT,
            access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY,
            limit=1,
            excluded_sha256=frozenset({"B" * 64}),
        )
    )
    assert len(result) == 1 and result[0].model_name == "openmodeldb title"
    assert tuple(offer.reference.provider_id for offer in result[0].offers) == (
        "openmodeldb",
        "civitai",
    )
    assert (
        first.requests
        == second.requests
        == [
            (CONTEXT, ModelSuggestionAccessPolicy.PUBLIC_ONLY, 1, frozenset({"b" * 64}))
        ]
    )
    assert not unsupported.requests
    assert engine.browse_urls(CONTEXT) == (
        ("openmodeldb", "https://example.test/openmodeldb"),
        ("civitai", "https://example.test/civitai"),
    )


@pytest.mark.parametrize("ids", [(), ("",), (" ",), ("civitai", "civitai")])
def test_rejects_empty_or_competing_provider_registrations(
    ids: tuple[str, ...],
) -> None:
    """Prevent ambiguous source ownership before any external operation runs."""
    with pytest.raises(ValueError, match="non-empty and unique"):
        ModelSuggestionEngine(tuple(ProviderFixture(identity, ()) for identity in ids))


def test_rejects_multi_source_or_foreign_provider_responses() -> None:
    """Require each source to own exactly the offer it returns."""
    first, foreign = suggestion("openmodeldb", "a" * 64), suggestion(
        "civitai", "a" * 64
    )
    for item, reason in (
        (foreign, "another source"),
        (replace(first, offers=(*first.offers, *foreign.offers)), "one owned offer"),
    ):
        engine = ModelSuggestionEngine((ProviderFixture("openmodeldb", (item,)),))
        with pytest.raises(ValueError, match=reason):
            asyncio.run(
                engine.suggest(
                    CONTEXT,
                    access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                    limit=8,
                )
            )
    with pytest.raises(ValueError, match="positive"):
        asyncio.run(
            engine.suggest(
                CONTEXT, access_policy=ModelSuggestionAccessPolicy.CURRENT_USER, limit=0
            )
        )


def test_routes_the_reviewed_offer_and_rejects_absent_or_unregistered_sources() -> None:
    """Preserve exact provider selection instead of falling back to another artifact URL."""
    first, second = ProviderFixture("openmodeldb", ()), ProviderFixture("civitai", ())
    primary, secondary = suggestion("openmodeldb", "a" * 64), suggestion(
        "civitai", "a" * 64
    )
    reviewed = replace(primary, offers=(*primary.offers, *secondary.offers))
    engine = ModelSuggestionEngine((first, second))
    progress: list[tuple[int, int]] = []
    result = asyncio.run(
        engine.acquire(
            reviewed,
            provider_id="civitai",
            progress=lambda current, total: progress.append((current, total)),
        )
    )
    assert result.offer == secondary.primary_offer.artifact
    assert (
        second.acquisitions == [(reviewed, secondary.primary_offer)]
        and not first.acquisitions
    )
    assert progress == [(7, 7)]
    asyncio.run(engine.acquire(reviewed))
    assert first.acquisitions == [(reviewed, primary.primary_offer)]
    with pytest.raises(ValueError, match="no offer"):
        asyncio.run(engine.acquire(reviewed, provider_id="missing"))
    with pytest.raises(ValueError, match="No registered"):
        asyncio.run(
            ModelSuggestionEngine((first,)).acquire(reviewed, provider_id="civitai")
        )


def test_provider_failures_and_cancellation_do_not_substitute_another_source(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep provider errors observable and cancellation authoritative across the routing boundary."""
    first, second = ProviderFixture("openmodeldb", ()), ProviderFixture("civitai", ())
    engine = ModelSuggestionEngine((first, second))

    async def failed_suggest(
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str],
    ) -> tuple[ModelSuggestion, ...]:
        """Represent an external provider outage without changing engine-owned policy."""
        raise OSError("provider offline")

    monkeypatch.setattr(first, "suggest", failed_suggest)
    with pytest.raises(OSError, match="provider offline"):
        asyncio.run(
            engine.suggest(
                CONTEXT, access_policy=ModelSuggestionAccessPolicy.CURRENT_USER, limit=8
            )
        )
    assert not second.requests

    async def cancelled_acquire(
        selected: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Represent cancellation raised by the shared acquisition owner."""
        raise asyncio.CancelledError()

    monkeypatch.setattr(first, "acquire", cancelled_acquire)
    with pytest.raises(asyncio.CancelledError):
        asyncio.run(engine.acquire(suggestion("openmodeldb", "a" * 64)))
    assert not second.acquisitions
