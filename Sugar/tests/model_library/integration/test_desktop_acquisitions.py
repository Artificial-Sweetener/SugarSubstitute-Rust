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
"""Prove desktop recipes use shared verified installation, progress, and persistence."""

from __future__ import annotations

from tests.model_library.support.download_access import (
    fixture_download_authorization,
    AccessTransportSession,
)

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from tests.model_library.support.enrichment import ObservedProvider

import asyncio
from dataclasses import replace
import hashlib
from pathlib import Path

import aiohttp
import pytest

from sugarcubes.model_library.application.requested_artifacts import (
    RequestedModelArtifacts,
)
from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
)
from sugarcubes.model_library.infrastructure.provider_credentials import civitai_api_key
from tests.model_library.support.model_roots import FixtureFileRoots


class AcquisitionProviderFixture:
    """Replace only the provider boundary with safe tiny-file acquisition evidence."""

    def __init__(self, kind: str = "loras") -> None:
        """Keep verified offers and observed credentials independent of production APIs."""
        self.kind = kind
        self.credentials: list[str | None] = []
        self.version = ProviderModelVersion(
            "civitai",
            "10",
            "20",
            "Fixture",
            "v1",
            "SDXL",
            "https://civitai.com/models/10",
            "",
            (),
            None,
            model_type="LORA" if kind == "loras" else "Checkpoint",
        )

    async def requested_version(self, sha256: str) -> ProviderModelVersion | None:
        """Return a known content identity without reading installed model files."""
        self.credentials.append(civitai_api_key())
        return self.version

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return one freshly validated tiny artifact, including its real content digest."""
        return (
            ModelProviderRelease(
                self.version,
                "2026-10-03",
                (
                    ModelArtifactOffer(
                        "30",
                        "fixture.safetensors",
                        hashlib.sha256(b"fixture").hexdigest(),
                        7,
                        "https://civitai.com/api/download/models/20",
                        self.kind,
                    ),
                ),
            ),
        )


def recipe() -> dict[str, object]:
    """Use the established desktop wire names and nested destination preferences."""
    return {
        "kind": "loras",
        "sha256": hashlib.sha256(b"fixture").hexdigest(),
        "downloadUrl": "https://civitai.com/api/download/models/20",
        "fileName": "fixture.safetensors",
        "fileType": "Model",
        "metadataFormat": "SafeTensor",
        "pickleScanResult": "Success",
        "virusScanResult": "Success",
        "downloadPathPattern": "{base_model}/{file_name}",
        "downloadPathTokens": {"baseModel": "SDXL 1.0"},
        "apiKey": "fixture-secret",
    }


def test_missing_model_download_opt_out_preserves_direct_picker_acquisitions(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Reject automatic recipe acquisition while preserving explicit CivitAI and OpenModelDB actions."""
    session = AccessTransportSession()
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Use production runtime preferences and disk/HTTP owners with seven-byte artifacts."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        installer = runtime.desktop_acquisitions.installer
        fixture = AcquisitionProviderFixture()
        offer = (await fixture.releases("10"))[0].offers[0]
        try:
            runtime.preferences.configure({"downloads_enabled": False})
            with pytest.raises(ValueError, match="downloads are disabled"):
                runtime.desktop_acquisitions.start(recipe())
            assert not session.requests
            assert not runtime.repository.list_entries()
            assert not tuple(tmp_path.rglob("*.safetensors"))
            assert not tuple(tmp_path.rglob(".sugarcubes-*.reserve"))
            openmodeldb = replace(
                offer,
                provider="openmodeldb",
                file_name="fixture.pth",
                kind="upscale_models",
                download_url="https://huggingface.co/fixture/model/resolve/main/fixture.pth",
            )
            upscalers = ModelArtifactDownloads(
                FixtureFileRoots(tmp_path, kind="upscale_models"),
                lambda: None,
                fixture_download_authorization(),
            )
            assert await upscalers.install(openmodeldb) == "fixture.pth"
            assert len(session.requests) == 1
            assert (tmp_path / "fixture.pth").read_bytes() == b"fixture"
            assert await installer.install(offer) == "fixture.safetensors"
            assert len(session.requests) == 3
            assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
            assert not runtime.preferences.downloads_enabled()
            runtime.preferences.configure({"downloads_enabled": True})
            acquisitions = replace(
                runtime.desktop_acquisitions, artifacts=RequestedModelArtifacts(fixture)
            )
            job = acquisitions.start(recipe())
            await runtime.jobs.wait(job.job_id)
            assert job.status == "completed"
            assert (tmp_path / "SDXL/fixture.safetensors").read_bytes() == b"fixture"
            assert len(session.requests) == 5
            assert runtime.preferences.missing_model_lookup_enabled()
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_desktop_acquisition_commits_shared_hash_and_progress_without_overwrite(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Install fixture bytes, coalesce requests, and publish one verified catalog identity."""
    session = AccessTransportSession()
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = AcquisitionProviderFixture()

    async def exercise() -> None:
        """Keep model weights and real provider requests out of installation proof."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        acquisitions = replace(
            runtime.desktop_acquisitions, artifacts=RequestedModelArtifacts(provider)
        )
        job = acquisitions.start(recipe())
        assert acquisitions.start(recipe()) is job
        assert "fixture-secret" not in str(job.to_payload())
        await runtime.jobs.wait(job.job_id)
        assert job.status == "completed" and not job.cancellable
        assert job.progress == {"bytesDownloaded": 7, "bytesTotal": 7}
        entries = runtime.repository.list_entries()
        assert (
            len(entries) == 1
            and entries[0].sha256 == hashlib.sha256(b"fixture").hexdigest()
        )
        assert entries[0].evidence.relative_path == "SDXL/fixture.safetensors"
        assert runtime.jobs.cancel(job.job_id) is job and job.status == "completed"
        second = acquisitions.start(recipe())
        await runtime.jobs.wait(second.job_id)
        assert second.status == "completed"
        third = acquisitions.start(recipe())
        await runtime.jobs.wait(third.job_id)
        assert third.status == "completed"
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
    assert (tmp_path / "SDXL/fixture.safetensors").read_bytes() == b"fixture"
    assert len(tuple((tmp_path / "SDXL").glob("*.safetensors"))) == 1
    assert len(session.requests) == 2
    assert not tuple(tmp_path.rglob("*.download"))
    assert provider.credentials == ["fixture-secret"] * 3
    assert civitai_api_key() != "fixture-secret"


def test_desktop_acquisition_rejects_provider_category_and_hash_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reject incompatible offers before transfer and remove failed SHA256 downloads."""
    session = AccessTransportSession(b"wrong!!")
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Read failure state from the real shared scheduler."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        acquisitions = replace(
            runtime.desktop_acquisitions,
            artifacts=RequestedModelArtifacts(
                AcquisitionProviderFixture("checkpoints")
            ),
        )
        job = acquisitions.start(recipe())
        await runtime.jobs.wait(job.job_id)
        assert job.status == "failed" and not session.requests
        acquisitions = replace(
            runtime.desktop_acquisitions,
            artifacts=RequestedModelArtifacts(AcquisitionProviderFixture()),
        )
        job = acquisitions.start(recipe())
        await runtime.jobs.wait(job.job_id)
        assert job.status == "failed" and "SHA256" in (job.error or "")
        assert not tuple(tmp_path.rglob("*.safetensors"))
        assert not tuple(tmp_path.rglob("*.download"))
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())


def test_browser_acquisition_uses_the_same_verified_publication_and_progress(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep browser downloads in the same fingerprint and publication policy as desktop recipes."""
    session = AccessTransportSession()
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    (tmp_path / "installed.safetensors").write_bytes(b"installed fixture")

    async def exercise() -> None:
        """Acquire tiny bytes through the browser's normal explicit release selection path."""
        roots = FixtureFileRoots(tmp_path)
        runtime = ModelLibraryRuntime(roots, enrichment_provider=ObservedProvider())
        runtime.catalog.refresh()
        entry = runtime.repository.list_entries()[0]
        digest = hashlib.sha256(b"installed fixture").hexdigest()
        runtime.repository.save_fingerprint(entry.evidence, digest)
        runtime.repository.save_provider_metadata(
            digest,
            (
                ProviderMetadataRecord(
                    ModelMetadata(
                        "Fixture",
                        base_model="SDXL",
                        provider="civitai",
                        provider_model_id="10",
                        provider_version_id="20",
                    )
                ),
            ),
        )
        runtime.releases = ModelReleaseService(
            runtime.repository,
            AcquisitionProviderFixture(),
            ModelArtifactDownloads(
                roots, lambda: None, fixture_download_authorization()
            ),
            runtime.model_updates.usage,
        )
        job = runtime.acquire_release(entry.evidence.location_id, "20", "30")
        await runtime.jobs.wait(job.job_id)
        assert job.status == "completed"
        assert job.progress == {"bytesDownloaded": 7, "bytesTotal": 7}
        installed = runtime.repository.find_entry(str(job.result["modelId"]))
        assert (
            installed is not None
            and installed.sha256 == hashlib.sha256(b"fixture").hexdigest()
        )
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
