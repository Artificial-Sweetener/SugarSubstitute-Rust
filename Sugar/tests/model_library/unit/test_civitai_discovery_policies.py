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
"""Safeguard exact family admission, shared image policy, and recommendation artifact eligibility."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import pytest

from sugarcubes.model_library.domain.model_families import (
    discovery_family,
    suggestion_family,
)
from sugarcubes.model_library.domain.model_suggestions import ModelSuggestionContext
from sugarcubes.model_library.domain.thumbnail_safety import (
    CivitaiThumbnailSafety,
    allows_civitai_image,
)
from sugarcubes.model_library.infrastructure.civitai_codec import parse_civitai_version
from sugarcubes.model_library.infrastructure.civitai_discovery_codec import (
    parse_family_candidates,
)
from sugarcubes.model_library.infrastructure.civitai_image_codec import (
    select_civitai_thumbnail,
)
from tests.model_library.support.civitai_discovery import (
    discovery_model,
    discovery_image,
)


@pytest.mark.parametrize(
    "role,family,accepted",
    [
        ("checkpoints", "SDXL", True),
        ("checkpoints", "Illustrious", True),
        ("diffusion_models", "anima", True),
        ("checkpoints", "anima", True),
        ("diffusion_models", "sdxl", True),
        ("loras", "sdxl", True),
        ("checkpoints", "flux.2", True),
        ("checkpoints", "", False),
    ],
)
def test_discovery_matches_declared_relationship_families(
    role: str, family: str, accepted: bool
) -> None:
    """Admit known provider families and addon roles without filename inference."""
    assert (
        suggestion_family(ModelSuggestionContext(role, family), model_relationships())
        is not None
    ) is accepted


@pytest.mark.parametrize(
    "policy,level,nsfw,expected",
    [
        (CivitaiThumbnailSafety.SFW_ONLY, " SFW ", False, True),
        (CivitaiThumbnailSafety.SFW_ONLY, 2, False, False),
        (CivitaiThumbnailSafety.ALLOW_SOFT, "racy", False, True),
        (CivitaiThumbnailSafety.ALLOW_SOFT, 2, True, False),
        (CivitaiThumbnailSafety.ALLOW_ALL, 4, True, True),
        (CivitaiThumbnailSafety.DISABLED, 1, False, False),
    ],
)
def test_shared_content_policy_matches_metadata_and_discovery(
    policy: CivitaiThumbnailSafety, level: str | int, nsfw: bool, expected: bool
) -> None:
    """Apply one safety decision to installed metadata and candidate portraits."""
    assert (
        allows_civitai_image(policy, image_type="image", nsfw=nsfw, level=level)
        is expected
    )
    image = discovery_image(1)
    image.update({"nsfwLevel": level, "nsfw": nsfw})
    selected = select_civitai_thumbnail([image], policy, discovery_portrait=True)
    version = parse_civitai_version(
        {
            "id": 1,
            "modelId": 1,
            "model": {"name": "Fixture"},
            "name": "Version",
            "images": [image],
        },
        policy,
    )
    assert (selected is not None) is expected
    assert (version.preview_url is not None) is expected
    assert (
        allows_civitai_image(policy, image_type="video", nsfw=nsfw, level=level)
        is expected
    )


@pytest.mark.parametrize(
    "change",
    [
        "wrong-type",
        "private",
        "wrong-base",
        "pickle",
        "virus",
        "file-type",
        "pth",
        "wrong-origin",
        "boolean-id",
    ],
)
def test_discovery_rejects_ineligible_provider_claims(change: str) -> None:
    """Use the shared artifact boundary for every category, identity, and scan rejection."""
    model = discovery_model(1, 11, "a" * 64)
    versions = model["modelVersions"]
    assert isinstance(versions, list) and isinstance(versions[0], dict)
    version: dict[str, object] = versions[0]
    files = version["files"]
    assert isinstance(files, list) and isinstance(files[0], dict)
    file: dict[str, object] = files[0]
    if change == "wrong-type":
        model["type"] = "LORA"
    elif change == "private":
        model["mode"] = "Archived"
    elif change == "wrong-base":
        version["baseModel"] = "SDXL 1.0"
    elif change == "pickle":
        file["pickleScanResult"] = "Pending"
    elif change == "virus":
        file["virusScanResult"] = "Pending"
    elif change == "file-type":
        file["type"] = "Training Data"
    elif change == "pth":
        file["name"] = "model.pth"
    elif change == "wrong-origin":
        file["downloadUrl"] = "https://127.0.0.1/secret"
    else:
        version["id"] = True
    family = discovery_family("sdxl", model_relationships())
    assert family is not None
    assert not parse_family_candidates(
        model,
        ModelSuggestionContext("checkpoints", "sdxl"),
        family,
        1,
        CivitaiThumbnailSafety.SFW_ONLY,
    )
