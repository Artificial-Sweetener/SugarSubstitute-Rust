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
"""Resolve compatible discovery from shared provider identities and retained host-family aliases."""

from __future__ import annotations

from dataclasses import dataclass

from .model_suggestions import ModelSuggestionContext
from .model_relationships import ModelRelationships


@dataclass(frozen=True, slots=True)
class ModelDiscoveryFamily:
    """Keep canonical family identity, artifact role, and recommendation base distinct."""

    family_id: str
    artifact_kind: str
    recommendation_base: str
    model_type: str
    linked_bases: frozenset[str]


DISCOVERY_FAMILIES = (
    ModelDiscoveryFamily(
        "sdxl",
        "checkpoints",
        "Illustrious",
        "Checkpoint",
        frozenset(
            {
                "Illustrious",
                "NoobAI",
                "Playground v2",
                "Pony",
                "SDXL 0.9",
                "SDXL 1.0",
                "SDXL 1.0 LCM",
                "SDXL Distilled",
                "SDXL Hyper",
                "SDXL Lightning",
                "SDXL Turbo",
            }
        ),
    ),
    ModelDiscoveryFamily(
        "anima",
        "diffusion_models",
        "Anima",
        "Checkpoint",
        frozenset({"Anima"}),
    ),
)


def discovery_family(
    target_model: str, relationships: ModelRelationships, artifact_kind: str = ""
) -> ModelDiscoveryFamily | None:
    """Resolve exact declared family identities for the requested resource category."""
    normalized = target_model.strip().casefold()
    legacy = next(
        (
            family
            for family in DISCOVERY_FAMILIES
            if normalized in (family.family_id, family.recommendation_base.casefold())
        ),
        None,
    )
    if legacy and (not artifact_kind or artifact_kind == legacy.artifact_kind):
        return legacy
    family = relationships.resolve(target_model)
    if family is None or not family.bases:
        return None
    kind = artifact_kind or (
        "diffusion_models"
        if family.architecture in {"FLUX", "FLUX.2", "Anima"}
        else "checkpoints"
    )
    if kind not in {"checkpoints", "diffusion_models", "unet", "loras", "embeddings"}:
        return None
    preferred = {
        "SDXL": "SDXL 1.0",
        "SD1": "SD 1.5",
        "SD2": "SD 2.1",
        "Flux1": "Flux.1 D",
    }.get(family.key, family.name)
    base = preferred if preferred in family.bases else family.bases[0]
    return ModelDiscoveryFamily(
        family.key,
        kind,
        base,
        {"loras": "LORA", "embeddings": "TextualInversion"}.get(kind, "Checkpoint"),
        frozenset(family.bases),
    )


def suggestion_family(
    context: ModelSuggestionContext, relationships: ModelRelationships
) -> ModelDiscoveryFamily | None:
    """Require declared family evidence before suggesting a resource in its host category."""
    family = discovery_family(
        context.family_id or "", relationships, context.artifact_kind
    )
    return family if family and family.artifact_kind == context.artifact_kind else None


def picker_suggestion_context(
    artifact_kind: str, target_model: str, relationships: ModelRelationships
) -> ModelSuggestionContext | None:
    """Resolve explicit family or curated upscaler discovery without filename inference."""
    if artifact_kind == "upscale_models":
        return ModelSuggestionContext(artifact_kind)
    family = discovery_family(target_model, relationships, artifact_kind)
    if family is None or family.artifact_kind != artifact_kind:
        return None
    return ModelSuggestionContext(artifact_kind, family.family_id)
