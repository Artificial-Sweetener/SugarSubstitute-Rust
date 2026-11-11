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
"""Prove animated provider previews use the same content policy and bounded CDN transport."""

import asyncio

import aiohttp
import pytest

from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.infrastructure.civitai_image_codec import (
    select_civitai_thumbnail,
)
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


@pytest.mark.parametrize("policy", list(CivitaiThumbnailSafety))
@pytest.mark.parametrize("nsfw,level", [(False, 1), (False, 2), (True, 4)])
def test_video_preview_admission_matches_still_image_preferences(
    policy: CivitaiThumbnailSafety, nsfw: bool, level: int
) -> None:
    """Apply identical configured content restrictions before downloading a video frame."""
    image = {
        "url": "https://image.civitai.com/preview",
        "type": "image",
        "nsfw": nsfw,
        "nsfwLevel": level,
    }
    expected = select_civitai_thumbnail([image], policy)
    assert select_civitai_thumbnail([{**image, "type": "video"}], policy) == expected
    assert select_civitai_thumbnail([{**image, "type": "audio"}], policy) is None


@pytest.mark.parametrize("mime", ["image/gif", "video/mp4", "video/webm"])
def test_animation_download_is_bounded_and_credential_free(
    monkeypatch: pytest.MonkeyPatch, mime: str
) -> None:
    """Admit supported media without changing the CDN host or credential boundary."""
    session = TransportSession(TransportResponse(200, b"media", mime))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    assert (
        asyncio.run(
            CivitaiTransport(lambda: "secret").read_preview(
                "https://image.civitai.com/preview"
            )
        )
        == b"media"
    )
    assert all("Authorization" not in headers for headers in session.headers)


@pytest.mark.parametrize(
    "mime", ["audio/mpeg", "text/html", "application/octet-stream"]
)
def test_unsupported_preview_media_is_rejected(
    monkeypatch: pytest.MonkeyPatch, mime: str
) -> None:
    """Keep unrecognized formats outside the thumbnail decoder."""
    session = TransportSession(TransportResponse(200, b"media", mime))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    with pytest.raises(ValueError, match="unsupported format"):
        asyncio.run(
            CivitaiTransport().read_preview("https://image.civitai.com/preview")
        )


def test_video_response_remains_subject_to_the_preview_byte_limit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Stop oversized video bodies before they reach the frame decoder."""
    session = TransportSession(
        TransportResponse(200, b"x" * (20 * 1024 * 1024 + 1), "video/mp4")
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    with pytest.raises(ValueError, match="size limit"):
        asyncio.run(
            CivitaiTransport().read_preview("https://image.civitai.com/preview")
        )
