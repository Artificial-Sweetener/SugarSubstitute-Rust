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
"""Prove CivitAI identifies a file with a rewritten header by the hash of its weights."""

from __future__ import annotations

import asyncio

import aiohttp
import pytest

from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from tests.model_library.support.civitai_discovery import DiscoveryTransportSession
from tests.model_library.support.provider_transport import TransportResponse

_FULL = "a" * 64
_WEIGHTS = "c" * 64
_BY_HASH = "/api/v1/model-versions/by-hash/"
_BY_FULL = _BY_HASH + _FULL
_BY_WEIGHTS = _BY_HASH + _WEIGHTS[:12]


def _version(*auto_v3: str) -> dict[str, object]:
    """Return a version answer whose files state the given weights hashes."""
    return {
        "id": 20,
        "modelId": 10,
        "name": "v2",
        "model": {"name": "Fixture"},
        "files": [{"hashes": {"AutoV3": stated}} for stated in auto_v3],
    }


class _WeightsHash:
    """Count how often a provider asks for the weights hash."""

    def __init__(self, value: str | None = _WEIGHTS) -> None:
        """Answer every request with one fixed hash."""
        self.value = value
        self.calls = 0

    async def __call__(self) -> str | None:
        """Record the request and return the fixed hash."""
        self.calls += 1
        return self.value


def _paths(session: DiscoveryTransportSession) -> list[str]:
    """List the provider paths requested, in order."""
    return [
        url.removeprefix("https://civitai.com") for url, _redirects in session.requests
    ]


def test_a_file_unknown_by_its_full_hash_is_found_by_its_weights_hash(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Ask again with the first twelve characters and accept a file stating that AutoV3."""
    session = DiscoveryTransportSession(
        {
            _BY_FULL: TransportResponse(404, b"{}"),
            _BY_WEIGHTS: _version("0123456789AB", "CCCCCCCCCCCC"),
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    weights = _WeightsHash()
    version = asyncio.run(CivitaiModelProvider().lookup(_FULL, weights))
    assert version is not None
    assert (version.provider, version.model_id, version.version_id) == (
        "civitai",
        "10",
        "20",
    )
    assert _paths(session) == [_BY_FULL, _BY_WEIGHTS]
    assert weights.calls == 1


@pytest.mark.parametrize(
    "answer",
    [
        _version("0123456789ab"),
        _version(),
        {"id": 20, "modelId": 10, "name": "v2", "model": {"name": "Fixture"}},
        _version("c" * 11),
    ],
    ids=["another-hash", "no-files", "files-absent", "shorter-hash"],
)
def test_a_weights_answer_that_does_not_state_the_asked_hash_is_rejected(
    monkeypatch: pytest.MonkeyPatch, answer: dict[str, object]
) -> None:
    """Refuse an answer none of whose files names the weights hash, and remember the miss."""
    session = DiscoveryTransportSession(
        {_BY_FULL: TransportResponse(404, b"{}"), _BY_WEIGHTS: answer}
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Look the same file up twice through one provider."""
        provider = CivitaiModelProvider()
        assert await provider.lookup(_FULL, _WeightsHash()) is None
        assert await provider.lookup(_FULL, _WeightsHash()) is None

    asyncio.run(exercise())
    assert _paths(session) == [_BY_FULL, _BY_WEIGHTS]


def test_a_lookup_a_person_asked_for_reaches_the_provider_after_a_remembered_miss(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Ask both hashes again on request; an automatic lookup inside the hour asks nothing."""
    session = DiscoveryTransportSession(
        {
            _BY_FULL: TransportResponse(404, b"{}"),
            _BY_WEIGHTS: TransportResponse(404, b"{}"),
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Miss, look up automatically, then look up as a person's refresh does."""
        provider = CivitaiModelProvider()
        assert await provider.lookup(_FULL, _WeightsHash()) is None
        assert await provider.lookup(_FULL, _WeightsHash()) is None
        assert _paths(session) == [_BY_FULL, _BY_WEIGHTS]
        assert await provider.lookup(_FULL, _WeightsHash(), ask_again=True) is None

    asyncio.run(exercise())
    assert _paths(session) == [_BY_FULL, _BY_WEIGHTS, _BY_FULL, _BY_WEIGHTS]


def test_the_weights_hash_is_never_computed_when_the_full_hash_matches(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Leave the file unread a second time when CivitAI knows its full hash."""
    session = DiscoveryTransportSession({_BY_FULL: _version()})
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    weights = _WeightsHash()
    version = asyncio.run(CivitaiModelProvider().lookup(_FULL, weights))
    assert version is not None and version.version_id == "20"
    assert weights.calls == 0
    assert _paths(session) == [_BY_FULL]


def test_a_file_without_a_weights_hash_stays_a_plain_miss(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Make no second request for a file that has no weights-only hash."""
    session = DiscoveryTransportSession({_BY_FULL: TransportResponse(404, b"{}")})
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    weights = _WeightsHash(None)
    assert asyncio.run(CivitaiModelProvider().lookup(_FULL, weights)) is None
    assert weights.calls == 1
    assert _paths(session) == [_BY_FULL]


def test_a_weights_hash_civitai_does_not_know_is_a_miss(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Report a confirmed miss when both the full hash and the weights hash are unknown."""
    session = DiscoveryTransportSession(
        {
            _BY_FULL: TransportResponse(404, b"{}"),
            _BY_WEIGHTS: TransportResponse(404, b"{}"),
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    assert asyncio.run(CivitaiModelProvider().lookup(_FULL, _WeightsHash())) is None
    assert _paths(session) == [_BY_FULL, _BY_WEIGHTS]
