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
"""Keep prepared-source storage failures on the recoverable provider preview path."""

from __future__ import annotations

import asyncio
from pathlib import Path
import sqlite3

import pytest

from sugarcubes.model_library.application.enrichment_service import (
    ModelPreviewUnavailableError,
)
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.provider_catalog_cache import (
    ProviderCatalogCache,
)
from sugarcubes.model_library.infrastructure.provider_previews import (
    ModelProviderPreviewPreparer,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)


def test_busy_source_database_preserves_recoverable_preview_failure(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep a cache write failure from discarding verified model metadata during enrichment."""

    async def content(_transport: CivitaiTransport, url: str) -> bytes:
        """Replace only provider transport; the real preparation enters its SQLite lease."""
        assert url == "https://image.civitai.com/fixture.jpg"
        return b"fixture source"

    def busy(*_args: object, **_kwargs: object) -> sqlite3.Connection:
        """Fail only the external SQLite connection boundary after runtime composition."""
        raise sqlite3.OperationalError("database is locked")

    assets = ModelThumbnailAssets(tmp_path / "assets")
    previews = ModelProviderPreviewPreparer(
        CivitaiModelProvider(),
        OpenModelDbProvider(ProviderCatalogCache(tmp_path / "catalog.json")),
        ModelThumbnailPreparer(assets, origin="civitai"),
        ModelThumbnailPreparer(assets, origin="openmodeldb"),
    )
    monkeypatch.setattr(CivitaiTransport, "read_preview", content)
    monkeypatch.setattr(sqlite3, "connect", busy)
    with pytest.raises(ModelPreviewUnavailableError) as error:
        asyncio.run(previews.prepare("https://image.civitai.com/fixture.jpg"))
    assert isinstance(error.value.__cause__, sqlite3.OperationalError)
    assert not tuple((tmp_path / "assets").glob("*.webp"))
