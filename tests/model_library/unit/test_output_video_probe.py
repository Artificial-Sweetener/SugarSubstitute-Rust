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
"""Verify bounded stream metadata probing reuses the existing portable video owner."""

from io import BytesIO

import pytest

from sugarcubes.model_library.infrastructure.preview_frame import probe_video_dimensions
from tests.model_library.support.preview_media import animated_preview


@pytest.mark.parametrize("format", ["mp4", "webm"])
def test_output_video_metadata_matches_real_encoded_container(format: str) -> None:
    """Inspect real installed-codec fixtures without decoding or retaining a preview image."""
    source = BytesIO(animated_preview(format))
    assert probe_video_dimensions(source, "mov" if format == "mp4" else "matroska") == (
        64,
        96,
    )


def test_invalid_video_metadata_is_rejected() -> None:
    """Keep malformed containers outside the artifact receipt boundary."""
    with pytest.raises(ValueError):
        probe_video_dimensions(BytesIO(b"not a video"), "mov")
