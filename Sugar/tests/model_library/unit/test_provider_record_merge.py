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
"""Prove a later lookup only adds to what a model already learned from its providers."""

from __future__ import annotations

import pytest

from sugarcubes.model_library.domain.models import (
    ModelMetadata,
    PreviewCandidate,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.provider_metadata import (
    ProviderMetadataRecord,
    merge_previews,
    merge_provider_records,
)
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.thumbnail_safety import (
    EXPLICIT_TIER,
    SAFE_TIER,
    SOFT_TIER,
)

_STORED_IMAGE = (ThumbnailReference("a" * 64, "standard", 128, 128, "image/webp"),)
_FETCHED = PreviewCandidate(
    "https://image.civitai.com/withdrawn.jpg", EXPLICIT_TIER, _STORED_IMAGE
)
_UNFETCHED = PreviewCandidate("https://image.civitai.com/still.jpg", SOFT_TIER)
_NEW = PreviewCandidate("https://image.civitai.com/new.jpg", SAFE_TIER)

_OPENMODELDB = ProviderMetadataRecord(
    ModelMetadata(
        "Reference title",
        provider="openmodeldb",
        provider_model_id="4x-Fixture",
        provider_version_id="4x-Fixture",
    )
)
_CIVITAI = ProviderMetadataRecord(
    ModelMetadata(
        "Learned title",
        version="v1",
        provider="civitai",
        provider_model_id="10",
        provider_version_id="20",
        trained_words=("learned",),
        thumbnails=_STORED_IMAGE,
        thumbnail_tier=EXPLICIT_TIER,
        previews=(_FETCHED, _UNFETCHED),
    ),
    "civitai",
)


def _civitai_answer() -> ProviderModelVersion:
    """Return a later CivitAI answer with new words and a changed image offer."""
    return ProviderModelVersion(
        "civitai",
        "10",
        "20",
        "Renamed title",
        "v1-renamed",
        "SDXL 1.0",
        "https://civitai.com/models/10?modelVersionId=20",
        "",
        ("renamed",),
        _NEW.url,
        previews=(_NEW, _UNFETCHED),
    )


@pytest.mark.parametrize("replace_labels", [False, True], ids=["automatic", "manual"])
def test_a_lookup_keeps_unreturned_records_the_stored_image_and_fetched_offers(
    replace_labels: bool,
) -> None:
    """Keep the silent provider, the first image, and a fetched image the offer dropped."""
    merged = merge_provider_records(
        (_OPENMODELDB, _CIVITAI), (_civitai_answer(),), replace_labels=replace_labels
    )
    assert len(merged) == 2
    assert merged[0] == _OPENMODELDB
    civitai = merged[1]
    assert civitai.thumbnail_origin == "civitai"
    assert civitai.metadata.thumbnails == _STORED_IMAGE
    assert civitai.metadata.thumbnail_tier == EXPLICIT_TIER
    assert civitai.metadata.previews == (_NEW, _UNFETCHED, _FETCHED)
    assert (
        civitai.metadata.provider_model_id,
        civitai.metadata.provider_version_id,
    ) == (
        "10",
        "20",
    )
    assert (
        civitai.metadata.title,
        civitai.metadata.version,
        civitai.metadata.trained_words,
    ) == (
        ("Renamed title", "v1-renamed", ("renamed",))
        if replace_labels
        else ("Learned title", "v1", ("learned",))
    )


def test_a_provider_that_answers_for_the_first_time_is_added_after_stored_records() -> (
    None
):
    """Add a new provider record without an image and leave the stored order alone."""
    merged = merge_provider_records(
        (_OPENMODELDB,), (_civitai_answer(),), replace_labels=False
    )
    assert merged[0] == _OPENMODELDB
    assert merged[1] == ProviderMetadataRecord(_civitai_answer().metadata(()))
    assert merged[1].metadata.thumbnails == ()
    assert merged[1].metadata.previews == (_NEW, _UNFETCHED)


def test_a_lookup_that_returns_nothing_leaves_every_stored_record() -> None:
    """Return the stored records unchanged when no provider answered."""
    assert merge_provider_records(
        (_OPENMODELDB, _CIVITAI), (), replace_labels=True
    ) == (_OPENMODELDB, _CIVITAI)


def test_merged_offers_follow_the_provider_and_keep_fetched_files() -> None:
    """Take the current offer in its order, keep fetched files, and drop unfetched leftovers."""
    refetched = PreviewCandidate(_NEW.url, SAFE_TIER, _STORED_IMAGE)
    assert merge_previews((_UNFETCHED, refetched, _FETCHED), (_NEW,)) == (
        refetched,
        _FETCHED,
    )
    assert merge_previews((), (_NEW, _UNFETCHED)) == (_NEW, _UNFETCHED)
