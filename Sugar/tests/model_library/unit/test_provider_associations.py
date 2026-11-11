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
"""Prove exact-hash provider associations survive presentation changes and catalog restart."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import asyncio
from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.model_library.application.provider_registry import (
    ModelMetadataProviders,
    RegisteredModelProvider,
)
from sugarcubes.model_library.domain.models import (
    ModelEvidence,
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_links import ModelProviderLink
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.metadata_codec import (
    decode_metadata,
    encode_metadata,
)


class ExactProvider:
    """Observe the validated provider seam rather than guessing identity from filenames."""

    def __init__(self, match: ProviderModelVersion | None) -> None:
        """Bind a deterministic match or confirmed absence."""
        self.match = match
        self.hashes: list[str] = []

    async def lookup(self, sha256: str) -> ProviderModelVersion | None:
        """Record exact content evidence for each supported lookup."""
        self.hashes.append(sha256)
        return self.match


def test_every_supported_hash_match_is_retained_in_reference_order() -> None:
    """Choose OpenModelDB labels while retaining CivitAI and collapsing duplicate registrations."""
    first = ProviderModelVersion(
        "openmodeldb",
        "4x-Fixture",
        "4x-Fixture",
        "Reference title",
        "",
        "span",
        "https://openmodeldb.info/models/4x-Fixture",
        "",
        (),
        None,
    )
    second = replace(
        first,
        provider="civitai",
        model_id="10",
        version_id="20",
        title="Secondary title",
        page_url="https://civitai.com/models/10",
    )
    upscaler, civitai = ExactProvider(first), ExactProvider(second)
    registry = ModelMetadataProviders(
        (
            RegisteredModelProvider(frozenset({"upscale_models"}), upscaler),
            RegisteredModelProvider(frozenset(), civitai),
            RegisteredModelProvider(frozenset(), civitai),
        )
    )
    result = asyncio.run(registry.lookup("a" * 64, "upscale_models"))
    assert result == (first, second)
    assert [
        (link.provider_id, link.model_id, link.version_id)
        for version in result
        for link in version.metadata(()).links()
    ] == [("openmodeldb", "4x-Fixture", "4x-Fixture"), ("civitai", "10", "20")]
    assert upscaler.hashes == ["a" * 64] and civitai.hashes == ["a" * 64, "a" * 64]
    asyncio.run(registry.lookup("b" * 64, "loras"))
    assert upscaler.hashes == ["a" * 64]


def test_links_survive_user_labels_thumbnails_and_restart_but_not_changed_content(
    tmp_path: Path,
) -> None:
    """Keep provider identity independent from user presentation and stale physical evidence."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = ModelEvidence(
        "upscale_models", "same.pth", "primary", "same.pth", 12, 100
    )
    repository.reconcile((evidence,), None)
    repository.save_fingerprint(evidence, "a" * 64)
    links = (
        ModelProviderLink(
            "openmodeldb",
            "OpenModelDB",
            "4x-Fixture",
            "4x-Fixture",
            "https://openmodeldb.info/models/4x-Fixture",
        ),
        ModelProviderLink(
            "civitai", "CivitAI", "10", "20", "https://civitai.com/models/10"
        ),
    )
    repository.save_provider_metadata(
        "a" * 64,
        (ProviderMetadataRecord(ModelMetadata("Remote", provider_links=links)),),
    )
    repository.save_user_metadata("a" * 64, ModelMetadata("<b>My label</b>"))
    thumbnail = ThumbnailReference("b" * 64, "standard", 128, 192, "image/webp")
    repository.save_thumbnail_choice(evidence, (thumbnail,))
    reopened = SqliteModelCatalogRepository(tmp_path)
    entry = reopened.find_entry(evidence.location_id)
    assert entry is not None and entry.metadata is not None
    assert entry.evidence.value == "same.pth"
    assert entry.metadata.title == "<b>My label</b>" and entry.metadata.thumbnails == (
        thumbnail,
    )
    assert entry.metadata.links() == links
    assert entry.to_payload()["providerLinks"] == [link.to_payload() for link in links]
    reopened.reconcile((replace(evidence, modified_ns=101),), None)
    assert reopened.list_entries()[0].to_payload()["providerLinks"] == []


@pytest.mark.parametrize(
    "url",
    [
        "javascript:bad",
        "https://user@civitai.com/models/10",
        "https://127.0.0.1/models/10",
        "https://civitai.com:444/models/10",
    ],
)
def test_untrusted_association_urls_never_enter_storage(url: str) -> None:
    """Validate saved provider pages before either frontend can expose navigation."""
    metadata = ModelMetadata(
        "Fixture",
        provider_links=(ModelProviderLink("civitai", "CivitAI", "10", "20", url),),
    )
    with pytest.raises(ValueError, match="provider page"):
        decode_metadata(encode_metadata(metadata))


def test_existing_metadata_schema_preserves_primary_links() -> None:
    """Read persisted public schema one without rewriting legacy metadata."""
    metadata = decode_metadata(
        '{"title":"Fixture","provider":"civitai","providerUrl":"https://civitai.com/models/10","providerModelId":"10","providerVersionId":"20"}'
    )
    assert metadata.links()[0].provider_name == "CivitAI"
