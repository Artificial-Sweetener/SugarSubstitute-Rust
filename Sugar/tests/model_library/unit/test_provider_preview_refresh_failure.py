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
"""Report a failed provider preview refresh plainly while keeping the previous preview."""

from __future__ import annotations

import asyncio
import hashlib
from io import BytesIO
import logging
from pathlib import Path

import aiohttp
from PIL import Image
from multidict import CIMultiDict, CIMultiDictProxy
import pytest
from yarl import URL

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.enrichment_service import (
    ModelEnrichmentService,
)
from sugarcubes.model_library.application.fingerprint_service import (
    ModelFingerprintService,
)
from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.openmodeldb_codec import (
    parse_openmodeldb_catalog,
)
from sugarcubes.model_library.infrastructure.provider_catalog_cache import (
    ProviderCatalogCache,
)
from sugarcubes.model_library.infrastructure.provider_previews import (
    ModelProviderPreviewPreparer,
)
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots

_SERVICE_LOGGER = "sugarcubes.model_library.application.enrichment_service"


def _openmodeldb_version(digest: str) -> ProviderModelVersion:
    """Return a real OpenModelDB match whose preview comes from a reviewed origin."""
    catalog = parse_openmodeldb_catalog(
        {
            "4x-Fixture": {
                "name": "Fresh title",
                "architecture": "span",
                "resources": [
                    {
                        "type": "pth",
                        "sha256": digest,
                        "size": 23,
                        "urls": ["https://github.com/example/fixture.pth"],
                    }
                ],
                "thumbnail": {"SR": "/thumbs/fixture.png"},
            }
        }
    )
    match = catalog.resource_for_sha256(digest)
    assert match is not None
    version = match[0].provider_version(match[1])
    assert version.preview_url == "https://openmodeldb.info/thumbs/fixture.png"
    return version


def _refresh(
    tmp_path: Path,
    previews: ModelProviderPreviewPreparer,
    fallback: tuple[ThumbnailReference, ...],
) -> tuple[dict[str, object], tuple[ThumbnailReference, ...]]:
    """Refresh one upscaler that already has a prepared preview, returning what was kept."""
    content = b"fixture-only-no-weights"
    (tmp_path / "fixture.pth").write_bytes(content)
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    digest = hashlib.sha256(content).hexdigest()
    repository.save_fingerprint(repository.list_entries()[0].evidence, digest)
    repository.save_provider_metadata(
        digest,
        (ProviderMetadataRecord(ModelMetadata(title="Old", thumbnails=fallback)),),
    )
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        ObservedProvider(_openmodeldb_version(digest)),
        previews,
    )
    result = asyncio.run(service.refresh(repository.list_entries()[0])).to_payload()
    metadata = repository.list_entries()[0].metadata
    assert metadata is not None
    return result, metadata.thumbnails


def _preparer(
    tmp_path: Path,
) -> tuple[ModelProviderPreviewPreparer, tuple[ThumbnailReference, ...]]:
    """Compose the production preview preparer and the preview prepared by an earlier refresh."""
    assets = ModelThumbnailAssets(tmp_path / "assets")
    openmodeldb_images = ModelThumbnailPreparer(assets, origin="openmodeldb")
    stream = BytesIO()
    Image.new("RGB", (96, 144), "navy").save(stream, "PNG")
    fallback = openmodeldb_images.prepare(stream.getvalue())
    preparer = ModelProviderPreviewPreparer(
        CivitaiModelProvider(),
        OpenModelDbProvider(ProviderCatalogCache(tmp_path / "catalog.json")),
        ModelThumbnailPreparer(assets, origin="civitai"),
        openmodeldb_images,
    )
    return preparer, fallback


def _refresh_warnings(caplog: pytest.LogCaptureFixture) -> list[logging.LogRecord]:
    """Return the enrichment service's preview refresh warnings."""
    return [
        record
        for record in caplog.records
        if record.name == _SERVICE_LOGGER
        and getattr(record, "operation", None) == "model_preview_refresh"
    ]


@pytest.mark.parametrize(
    ("failure", "cause"),
    [
        (
            aiohttp.ServerTimeoutError("Timeout on reading data from socket"),
            "the provider did not answer in time",
        ),
        (
            aiohttp.ClientResponseError(
                aiohttp.RequestInfo(
                    URL("https://openmodeldb.info/thumbs/fixture.png"),
                    "GET",
                    CIMultiDictProxy(CIMultiDict()),
                ),
                (),
                status=503,
            ),
            "the provider answered HTTP 503",
        ),
        (
            ValueError("The OpenModelDB preview is unavailable or unsupported."),
            "The OpenModelDB preview is unavailable or unsupported.",
        ),
    ],
    ids=["timeout", "http-status", "unsupported-response"],
)
def test_expected_network_failure_logs_one_plain_warning_without_a_traceback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
    failure: Exception,
    cause: str,
) -> None:
    """Name the operation and its plain cause once, keeping the previous preview."""

    async def unreachable(_provider: OpenModelDbProvider, url: str) -> bytes:
        """Fail only the external provider transport, as an offline network does."""
        assert url == "https://openmodeldb.info/thumbs/fixture.png"
        raise failure

    previews, fallback = _preparer(tmp_path)
    monkeypatch.setattr(OpenModelDbProvider, "read_preview", unreachable)
    caplog.set_level(logging.WARNING)
    result, thumbnails = _refresh(tmp_path, previews, fallback)
    assert result["status"] == "found" and "warning" in result
    assert thumbnails == fallback
    (warning,) = _refresh_warnings(caplog)
    assert warning.levelno == logging.WARNING
    assert warning.exc_info is None and warning.exc_text is None
    assert getattr(warning, "cause") == cause
    assert getattr(warning, "provider") == "openmodeldb"
    message = warning.getMessage()
    assert "operation=model_preview_refresh" in message and f"cause={cause}" in message
    assert "Traceback" not in caplog.text


def test_unexpected_preparation_failure_keeps_its_traceback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Keep the traceback of a failure that is not an ordinary network outcome."""

    async def content(_provider: OpenModelDbProvider, _url: str) -> bytes:
        """Deliver provider bytes so the failure happens in local preparation."""
        return b"fixture source"

    def broken_storage(
        _preparer: ModelThumbnailPreparer, _content: bytes
    ) -> tuple[ThumbnailReference, ...]:
        """Fail the local asset store the way a damaged disk does."""
        raise OSError("fixture storage failure")

    previews, fallback = _preparer(tmp_path)
    monkeypatch.setattr(OpenModelDbProvider, "read_preview", content)
    monkeypatch.setattr(ModelThumbnailPreparer, "prepare", broken_storage)
    caplog.set_level(logging.WARNING)
    result, thumbnails = _refresh(tmp_path, previews, fallback)
    assert "warning" in result and thumbnails == fallback
    (warning,) = _refresh_warnings(caplog)
    assert warning.exc_info is not None
    reported = warning.exc_info[1]
    assert reported is not None and isinstance(reported.__cause__, OSError)
    assert getattr(warning, "cause") == "OSError"
