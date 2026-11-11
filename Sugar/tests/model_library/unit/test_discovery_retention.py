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
"""Prove transient reviewed evidence remains bounded and follows reference context eligibility."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import pytest

from sugarcubes.model_library.domain.model_families import picker_suggestion_context
from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestionContext,
    ModelSuggestionPlan,
)
from sugarcubes.model_library.infrastructure.discovery_plans import (
    ReviewedDiscoveryPlans,
)


def test_reviewed_plan_expiry_capacity_and_release() -> None:
    """Reject expired and evicted tokens while preserving accepted immutable evidence."""
    now = [10.0]
    plans = ReviewedDiscoveryPlans(capacity=2, lifetime=5, clock=lambda: now[0])
    plan = ModelSuggestionPlan(ModelSuggestionContext("upscale_models"), (), ())
    first = plans.retain(plan, public_only=False)
    second = plans.retain(plan, public_only=True)
    assert first.plan_id != second.plan_id
    assert plans.get(second.plan_id).public_only
    third = plans.retain(plan, public_only=False)
    with pytest.raises(ValueError, match="expired"):
        plans.get(first.plan_id)
    plans.release(third.plan_id)
    with pytest.raises(ValueError, match="expired"):
        plans.get(third.plan_id)
    now[0] = 15
    with pytest.raises(ValueError, match="expired"):
        plans.get(second.plan_id)
    assert first.plan is plan
    plans.release(first.plan_id)


@pytest.mark.parametrize(
    "capacity,lifetime", [(0, 1), (1, 0), (1, -1), (1, float("inf")), (1, float("nan"))]
)
def test_invalid_retention_bounds_fail_closed(capacity: int, lifetime: float) -> None:
    """Require finite positive retention configuration rather than unbounded state."""
    with pytest.raises(ValueError, match="positive"):
        ReviewedDiscoveryPlans(capacity=capacity, lifetime=lifetime)


@pytest.mark.parametrize(
    "kind,target,expected",
    [
        ("upscale_models", "", ModelSuggestionContext("upscale_models")),
        ("upscale_models", "flux", ModelSuggestionContext("upscale_models")),
        ("checkpoints", "SDXL", ModelSuggestionContext("checkpoints", "sdxl")),
        ("checkpoints", "Illustrious", ModelSuggestionContext("checkpoints", "sdxl")),
        (
            "diffusion_models",
            "Anima",
            ModelSuggestionContext("diffusion_models", "anima"),
        ),
        ("checkpoints", "", None),
        ("checkpoints", "anima", ModelSuggestionContext("checkpoints", "Anima")),
        (
            "diffusion_models",
            "sdxl",
            ModelSuggestionContext("diffusion_models", "SDXL"),
        ),
        ("loras", "sdxl", ModelSuggestionContext("loras", "SDXL")),
        ("checkpoints", "flux", None),
        ("unknown", "sdxl", None),
    ],
)
def test_empty_picker_context_matches_reference_roles(
    kind: str,
    target: str,
    expected: ModelSuggestionContext | None,
) -> None:
    """Use explicit target metadata and primary artifact role, never filename guesses."""
    assert picker_suggestion_context(kind, target, model_relationships()) == expected
