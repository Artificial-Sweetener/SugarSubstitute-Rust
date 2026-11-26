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
"""Prove the thumbnail safety setting chooses among stored images without changing them."""

from __future__ import annotations

from dataclasses import replace

import pytest

from sugarcubes.model_library.domain.models import (
    ModelCatalogEntry,
    ModelEvidence,
    ModelMetadata,
    PreviewCandidate,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.thumbnail_presentation import (
    ThumbnailPresentation,
    present_thumbnail,
)
from sugarcubes.model_library.domain.thumbnail_safety import (
    EXPLICIT_TIER,
    SAFE_TIER,
    SOFT_TIER,
    CivitaiThumbnailSafety,
)
from sugarcubes.model_library.domain.thumbnail_sources import ThumbnailSourceOrigin

_EVIDENCE = ModelEvidence(
    "loras", "fixture.safetensors", "primary", "fixture.safetensors", 12, 100
)
_SHARP = (
    ThumbnailReference("a" * 64, "standard", 128, 192, "image/webp"),
    ThumbnailReference("b" * 64, "banner", 768, 160, "image/webp"),
)
_BLURRED = (
    ThumbnailReference("c" * 64, "standard-blurred", 128, 192, "image/webp"),
    ThumbnailReference("d" * 64, "banner-blurred", 768, 160, "image/webp"),
)
_EXPLICIT_URL = "https://image.civitai.com/explicit.jpg"
_SOFT_URL = "https://image.civitai.com/soft.jpg"
_SAFE_URL = "https://image.civitai.com/safe.jpg"


def _card(
    thumbnails: tuple[ThumbnailReference, ...] = _SHARP,
    *,
    previews: tuple[PreviewCandidate, ...] = (),
    origin: ThumbnailSourceOrigin | None = "civitai",
) -> ModelCatalogEntry:
    """Describe a card whose stored first image is explicit and owned by ``origin``."""
    return ModelCatalogEntry(
        _EVIDENCE,
        "e" * 64,
        ModelMetadata(
            "Fixture",
            provider="civitai",
            provider_model_id="10",
            provider_version_id="20",
            thumbnails=thumbnails,
            thumbnail_tier=EXPLICIT_TIER,
            previews=previews,
        ),
        origin,
    )


def test_an_unrestricted_setting_shows_a_stored_explicit_image() -> None:
    """Draw the stored files as prepared, with nothing owed."""
    assert present_thumbnail(
        _card(), CivitaiThumbnailSafety.ALLOW_ALL
    ) == ThumbnailPresentation(_SHARP)


def test_a_strict_setting_shows_nothing_and_says_the_image_is_filtered() -> None:
    """Report a filtered card when no stored or offered image is allowed."""
    offered_only_explicit = (
        PreviewCandidate(_EXPLICIT_URL, EXPLICIT_TIER, _SHARP),
        PreviewCandidate(_SOFT_URL, SOFT_TIER),
    )
    for card in (_card(), _card(previews=offered_only_explicit)):
        assert present_thumbnail(
            card, CivitaiThumbnailSafety.SFW_ONLY
        ) == ThumbnailPresentation(filtered=True)


def test_a_strict_setting_wants_the_first_safe_image_not_fetched_yet() -> None:
    """Name the first allowed offer to fetch, in provider order, instead of filtering."""
    first_safe = PreviewCandidate(_SAFE_URL, SAFE_TIER)
    card = _card(
        previews=(
            PreviewCandidate(_EXPLICIT_URL, EXPLICIT_TIER, _SHARP),
            PreviewCandidate(_SOFT_URL, SOFT_TIER),
            first_safe,
            PreviewCandidate("https://image.civitai.com/later.jpg", SAFE_TIER),
        )
    )
    shown = present_thumbnail(card, CivitaiThumbnailSafety.SFW_ONLY)
    assert shown == ThumbnailPresentation(wanted=first_safe)
    assert not shown.filtered and shown.thumbnails == ()
    soft = present_thumbnail(card, CivitaiThumbnailSafety.ALLOW_SOFT)
    assert soft.wanted == PreviewCandidate(_SOFT_URL, SOFT_TIER)


def test_a_strict_setting_shows_a_safe_image_that_was_already_fetched() -> None:
    """Choose a later stored image the setting allows over the explicit first one."""
    safe = (ThumbnailReference("f" * 64, "standard", 128, 128, "image/webp"),)
    card = _card(
        previews=(
            PreviewCandidate(_EXPLICIT_URL, EXPLICIT_TIER, _SHARP),
            PreviewCandidate(_SAFE_URL, SAFE_TIER, safe),
        )
    )
    assert present_thumbnail(
        card, CivitaiThumbnailSafety.SFW_ONLY
    ) == ThumbnailPresentation(safe)
    assert present_thumbnail(card, CivitaiThumbnailSafety.ALLOW_ALL).thumbnails == (
        _SHARP
    )


def test_the_blur_setting_shows_blurred_files_and_keeps_the_sharp_ones_for_reveal() -> (
    None
):
    """Draw blurred files under the ordinary roles; the sharp files are only a reveal."""
    shown = present_thumbnail(_card((*_SHARP, *_BLURRED)), CivitaiThumbnailSafety.BLUR)
    assert shown.blurred and not shown.filtered
    assert shown.thumbnails == (
        ThumbnailReference("c" * 64, "standard", 128, 192, "image/webp"),
        ThumbnailReference("d" * 64, "banner", 768, 160, "image/webp"),
    )
    assert shown.reveal == _SHARP
    assert shown.unblurred == () and shown.wanted is None


def test_the_blur_setting_asks_for_blurred_files_that_do_not_exist_yet() -> None:
    """Show nothing sharp while the blurred variants of a stored image are still owed."""
    shown = present_thumbnail(_card(), CivitaiThumbnailSafety.BLUR)
    assert shown == ThumbnailPresentation(unblurred=_SHARP, blurred=True)
    assert shown.thumbnails == () and shown.reveal == ()


def test_stored_blurred_files_are_never_drawn_by_a_setting_that_shows_sharp() -> None:
    """Leave blurred variants out of what an unrestricted setting draws."""
    shown = present_thumbnail(
        _card((*_SHARP, *_BLURRED)), CivitaiThumbnailSafety.ALLOW_ALL
    )
    assert shown == ThumbnailPresentation(_SHARP)


@pytest.mark.parametrize("origin", ["user", "local"])
@pytest.mark.parametrize(
    "policy",
    [policy for policy in CivitaiThumbnailSafety],
    ids=[policy.value for policy in CivitaiThumbnailSafety],
)
def test_an_image_a_person_owns_is_always_shown_sharp(
    origin: ThumbnailSourceOrigin, policy: CivitaiThumbnailSafety
) -> None:
    """Exempt user-chosen and beside-the-file images from every provider content setting."""
    card = _card(
        previews=(PreviewCandidate(_SAFE_URL, SAFE_TIER),),
        origin=origin,
    )
    assert present_thumbnail(card, policy) == ThumbnailPresentation(_SHARP)


def test_a_card_without_provider_labels_shows_and_wants_nothing() -> None:
    """Leave an unidentified model with an empty presentation that is not called filtered."""
    assert (
        present_thumbnail(
            replace(_card(), metadata=None), CivitaiThumbnailSafety.ALLOW_ALL
        )
        == ThumbnailPresentation()
    )
