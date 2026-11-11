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
"""Prove Substitute's update relevance, durable consent and dismissal rules with tiny files."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import asyncio
from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from datetime import datetime, timedelta
from pathlib import Path

import pytest

from sugarcubes.model_library.infrastructure.model_usage_store import (
    SqliteModelUsageRepository,
)
from sugarcubes.model_library.domain.models import ModelMetadata

from tests.model_library.support.model_updates import NOW, fixture


def test_updates_start_enabled_and_preserve_saved_choices(tmp_path: Path) -> None:
    """Enable new stores and retain deliberate preferences across usage changes and reopening."""
    subject, entry, provider = fixture(tmp_path)
    assert subject.usage.load().enabled
    subject.record_usage(entry)
    assert SqliteModelUsageRepository(tmp_path / "usage.sqlite3").load().enabled
    proposals = asyncio.run(subject.check_updates())
    assert len(proposals) == 1 and provider.calls == 1
    subject.set_enabled(False)
    subject.record_usage(entry)
    assert not SqliteModelUsageRepository(tmp_path / "usage.sqlite3").load().enabled
    assert asyncio.run(subject.check_updates()) == () and provider.calls == 1
    subject.set_enabled(True)
    assert SqliteModelUsageRepository(tmp_path / "usage.sqlite3").load().enabled


def test_update_checks_respect_opt_out_and_recent_known_usage(tmp_path: Path) -> None:
    """Honor saved disabling and avoid provider traffic for old dispatches."""
    subject, entry, provider = fixture(tmp_path)
    subject.set_enabled(False)
    subject.record_usage(entry)
    assert asyncio.run(subject.check_updates()) == () and provider.calls == 0
    subject.set_enabled(True)
    proposals = asyncio.run(subject.check_updates())
    assert len(proposals) == 1 and proposals[0].candidate.version.version_id == "21"
    subject.clock = lambda: NOW + timedelta(days=91)
    assert asyncio.run(subject.check_updates()) == () and provider.calls == 1


def test_update_dismissal_and_page_opt_out_persist_without_changing_selection(
    tmp_path: Path,
) -> None:
    """Dismiss one version, expose a later one, then suppress the entire provider page."""
    subject, entry, provider = fixture(tmp_path)
    subject.record_usage(entry)
    subject.set_enabled(True)
    assert subject.dismiss(entry.sha256 or "", "21")
    assert asyncio.run(subject.check_updates()) == ()
    provider.latest = "22"
    assert len(asyncio.run(subject.check_updates())) == 1
    assert subject.disable_model("10")
    before = provider.calls
    assert asyncio.run(subject.check_updates()) == () and provider.calls == before
    subject.record_usage(replace(entry, sha256="c" * 64))
    reopened = SqliteModelUsageRepository(tmp_path / "usage.sqlite3").load()
    assert reopened.enabled and all(
        record.updates_disabled_for_model for record in reopened.records
    )
    assert subject.catalog.list_entries()[0].evidence.value == "installed.safetensors"


def test_installed_candidates_and_changed_files_are_not_offered(tmp_path: Path) -> None:
    """Use catalog hashes for already-downloaded exclusion and avoid stale location identities."""
    subject, entry, provider = fixture(tmp_path)
    subject.record_usage(entry)
    subject.set_enabled(True)
    other = replace(
        entry.evidence, value="new.safetensors", relative_path="new.safetensors"
    )
    subject.catalog.reconcile((entry.evidence, other), None)
    subject.catalog.save_fingerprint(other, "a" * 64)
    assert asyncio.run(subject.check_updates()) == ()
    subject.catalog.save_fingerprint(entry.evidence, "d" * 64)
    calls = provider.calls
    assert asyncio.run(subject.check_updates()) == () and provider.calls == calls


def test_usage_retains_provider_evidence_and_atomic_counts(tmp_path: Path) -> None:
    """Keep known identity through transient metadata loss and serialize concurrent dispatch records."""
    subject, entry, _ = fixture(tmp_path)
    subject.record_usage(entry)
    with ThreadPoolExecutor(max_workers=4) as executor:
        list(executor.map(subject.record_usage, [replace(entry, metadata=None)] * 12))
    state = subject.usage.load()
    assert len(state.records) == 1
    record = state.records[0]
    assert (
        record.usage_count == 13
        and record.model_id == "10"
        and record.version_id == "20"
    )
    assert record.base_model == "SDXL"


def test_updates_use_retained_identity_when_catalog_metadata_is_unavailable(
    tmp_path: Path,
) -> None:
    """Keep exact dispatched evidence useful through a disposable metadata refresh."""
    subject, entry, provider = fixture(tmp_path)
    subject.record_usage(entry)
    subject.set_enabled(True)
    subject.catalog.save_provider_metadata(
        entry.sha256 or "", (ProviderMetadataRecord(ModelMetadata("Local title")),)
    )
    proposals = asyncio.run(subject.check_updates())
    assert len(proposals) == 1
    assert proposals[0].candidate.version.version_id == "21"
    assert provider.calls == 1
    assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"


@pytest.mark.parametrize(
    "metadata",
    [
        ModelMetadata(
            "Changed page",
            base_model="SDXL",
            provider="civitai",
            provider_model_id="11",
            provider_version_id="20",
        ),
        ModelMetadata(
            "Changed version",
            base_model="SDXL",
            provider="civitai",
            provider_model_id="10",
            provider_version_id="19",
        ),
        ModelMetadata(
            "Changed base",
            base_model="Flux",
            provider="civitai",
            provider_model_id="10",
            provider_version_id="20",
        ),
    ],
)
def test_updates_reject_catalog_evidence_that_contradicts_recorded_identity(
    tmp_path: Path,
    metadata: ModelMetadata,
) -> None:
    """Require a fresh dispatch before trusting a conflicting provider association."""
    subject, entry, provider = fixture(tmp_path)
    subject.record_usage(entry)
    subject.set_enabled(True)
    subject.catalog.save_provider_metadata(
        entry.sha256 or "", (ProviderMetadataRecord(metadata),)
    )
    assert asyncio.run(subject.check_updates()) == ()
    assert provider.calls == 0


def test_unknown_hash_or_invalid_clock_cannot_record_usage(tmp_path: Path) -> None:
    """Do not infer exact artifact identity from filenames or silently accept local-time timestamps."""
    subject, entry, _ = fixture(tmp_path)
    assert subject.record_usage(replace(entry, sha256=None)) is None
    subject.clock = lambda: datetime(2026, 10, 3)
    with pytest.raises(ValueError, match="timezone"):
        subject.record_usage(entry)
    assert subject.usage.load().records == ()


def test_failed_usage_transition_rolls_back_consent(tmp_path: Path) -> None:
    """Keep the committed opt-out when a malformed transition fails persistence validation."""
    subject, entry, _ = fixture(tmp_path)
    subject.set_enabled(False)
    subject.record_usage(entry)
    with pytest.raises(ValueError, match="count"):
        subject.usage.change(
            lambda state: replace(
                state, enabled=True, records=(replace(state.records[0], usage_count=0),)
            )
        )
    assert not subject.usage.load().enabled
