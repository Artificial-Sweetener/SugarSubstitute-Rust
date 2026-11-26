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
"""Name what a model's card shows where its image goes, so a blank card says why."""

from __future__ import annotations

from dataclasses import dataclass

from .models import ModelCatalogEntry, ThumbnailReference
from .thumbnail_presentation import ThumbnailPresentation

READY = "ready"
# A lookup or an image fetch is queued or running; the card flashes.
PENDING = "pending"
# The same, after a failure; the card flashes and says it is retrying.
RETRYING = "retrying"
# An image exists that the thumbnail safety setting excludes.
FILTERED = "filtered"
# Every provider was asked and none knows the file.
NOT_FOUND = "not_found"
# The model is known, or was never looked up, and has no image.
NONE = "none"


@dataclass(frozen=True, slots=True)
class CardImage:
    """Carry the files a card draws and the reason it draws none."""

    state: str
    thumbnails: tuple[ThumbnailReference, ...] = ()
    reveal: tuple[ThumbnailReference, ...] = ()
    blurred: bool = False

    def to_payload(self, *, reveal: bool) -> dict[str, object]:
        """Publish sharp files of a blurred image only when hover reveal is switched on."""
        return {
            "state": self.state,
            "blurred": self.blurred,
            "reveal": (
                [item.to_payload() for item in self.reveal]
                if reveal and self.blurred
                else []
            ),
        }


def card_image(
    entry: ModelCatalogEntry,
    shown: ThumbnailPresentation,
    *,
    asked: bool,
    lookup_owed: bool,
    failed: bool,
) -> CardImage:
    """Choose one state from what is stored, what is owed and what last went wrong.

    ``asked`` says the file's providers have answered for it; ``lookup_owed`` that one has not
    yet; ``failed`` that the owed work failed and waits for its retry.
    """
    if shown.thumbnails:
        return CardImage(READY, shown.thumbnails, shown.reveal, shown.blurred)
    if lookup_owed or shown.wanted is not None or shown.unblurred:
        return CardImage(RETRYING if failed else PENDING)
    if shown.filtered:
        return CardImage(FILTERED)
    known = entry.metadata is not None and bool(entry.metadata.links())
    return CardImage(NOT_FOUND if asked and not known else NONE)
