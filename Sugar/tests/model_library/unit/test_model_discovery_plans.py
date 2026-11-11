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
"""Prove immutable reviewed-plan membership and exact compatibility invariants."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

from dataclasses import replace

import pytest

from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionPlan,
    ModelSuggestionReference,
)
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer


def test_reviewed_plans_preserve_identity_and_reject_absent_selections() -> None:
    """Keep arbitrary provider or client values outside explicit reviewed membership."""
    context = ModelSuggestionContext("upscale_models")
    item = ModelSuggestion(
        context,
        "Model",
        "Version",
        None,
        "a" * 64,
        (
            ModelSuggestionOffer(
                ModelSuggestionReference(
                    "openmodeldb", "OpenModelDB", "model", "version"
                ),
                replace(
                    ModelArtifactOffer(
                        "artifact",
                        "model.pth",
                        "a" * 64,
                        7,
                        "https://github.com/fixture/model.pth",
                        "upscale_models",
                        "openmodeldb",
                    ),
                    access=ModelDownloadAccess.PUBLIC,
                ),
                "https://openmodeldb.info/models/fixture",
                None,
                1,
            ),
        ),
    )
    plan = ModelSuggestionPlan(context, (item,), ())
    assert plan.selected(item.identity) is item
    with pytest.raises(ValueError, match="reviewed plan"):
        plan.selected("upscale_models:" + "b" * 64)
    with pytest.raises(ValueError, match="unique compatible"):
        ModelSuggestionPlan(context, (item, item), ())
    foreign = replace(
        item,
        context=ModelSuggestionContext("loras"),
        offers=(
            replace(
                item.primary_offer,
                artifact=replace(item.primary_offer.artifact, kind="loras"),
            ),
        ),
    )
    with pytest.raises(ValueError, match="unique compatible"):
        ModelSuggestionPlan(context, (foreign,), ())
