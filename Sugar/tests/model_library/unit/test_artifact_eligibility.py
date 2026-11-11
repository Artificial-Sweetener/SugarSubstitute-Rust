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
"""Prove one provider eligibility policy for normal and curated model acquisition."""

from __future__ import annotations

from dataclasses import replace

import pytest

from sugarcubes.model_library.domain.artifact_eligibility import (
    ProviderFileEvidence,
    civitai_model_type,
    matches_civitai_kind,
    compatible_civitai_artifact_role,
    eligible_civitai_file,
    same_base_family,
)

SAFE_FILE = ProviderFileEvidence(
    "weights.safetensors", "Model", "SafeTensor", "Success", "Success", "a" * 64
)


@pytest.mark.parametrize(
    "kind,model_type",
    [
        ("checkpoints", "Checkpoint"),
        ("diffusion_models", "Checkpoint"),
        ("loras", "LORA"),
        ("vae", "VAE"),
        ("controlnet", "Controlnet"),
        ("upscale_models", "Upscaler"),
        ("embeddings", "TextualInversion"),
    ],
)
def test_exact_provider_types_and_standard_files(kind: str, model_type: str) -> None:
    """Require the declared type and clean SafeTensor evidence for every supported role."""
    assert civitai_model_type(kind) == model_type
    assert matches_civitai_kind(model_type, kind)
    assert not matches_civitai_kind("LoCon", kind)
    assert not matches_civitai_kind("", kind)
    assert eligible_civitai_file(SAFE_FILE, kind)
    assert not eligible_civitai_file(
        replace(SAFE_FILE, file_format="PickleTensor"), kind
    )
    assert not eligible_civitai_file(replace(SAFE_FILE, virus_scan="Danger"), kind)
    assert not eligible_civitai_file(replace(SAFE_FILE, pickle_scan="Pending"), kind)
    assert not eligible_civitai_file(
        replace(SAFE_FILE, file_type="Training Data"), kind
    )


def test_only_same_declared_type_can_adapt_host_artifact_roles() -> None:
    """Adapt Checkpoint provider files for diffusion roots without authorizing other categories."""
    assert compatible_civitai_artifact_role(
        "Checkpoint", "checkpoints", "diffusion_models"
    )
    assert not compatible_civitai_artifact_role(
        "Checkpoint", "loras", "diffusion_models"
    )
    assert not compatible_civitai_artifact_role("LORA", "checkpoints", "loras")
    assert not compatible_civitai_artifact_role("", "loras", "loras")
    assert not matches_civitai_kind("Checkpoint", "arbitrary_folder")


@pytest.mark.parametrize(
    "candidate,current,expected",
    [
        ("SDXL 1.0", " sdxl 1.0 ", True),
        ("SDXL", "SDXL 1.0", False),
        ("Flux", "SDXL", False),
        ("", "", False),
        (" ", "SDXL", False),
    ],
)
def test_base_compatibility_requires_exact_known_evidence(
    candidate: str, current: str, expected: bool
) -> None:
    """Keep family display, updates and acquisition independent of inferred compatibility."""
    assert same_base_family(candidate, current) is expected


def test_curated_upscaler_exception_requires_the_reviewed_hash_and_clean_pickle() -> (
    None
):
    """Match the reviewed OpenModelDB/CivitAI mirror rule without granting a general pickle exception."""
    evidence = replace(SAFE_FILE, name="reviewed.pth", file_format="PickleTensor")
    assert not eligible_civitai_file(evidence, "upscale_models")
    assert eligible_civitai_file(
        evidence, "upscale_models", curated_upscaler_sha256="a" * 64
    )
    assert not eligible_civitai_file(
        evidence, "loras", curated_upscaler_sha256="a" * 64
    )
    assert not eligible_civitai_file(
        evidence, "upscale_models", curated_upscaler_sha256="b" * 64
    )
    assert not eligible_civitai_file(
        replace(evidence, pickle_scan="Danger"),
        "upscale_models",
        curated_upscaler_sha256="a" * 64,
    )
    assert not eligible_civitai_file(
        replace(evidence, name="../reviewed.pth"),
        "upscale_models",
        curated_upscaler_sha256="a" * 64,
    )
    assert not eligible_civitai_file(
        evidence, "upscale_models", curated_upscaler_sha256="g" * 64
    )
    assert eligible_civitai_file(
        replace(SAFE_FILE, pickle_scan=""),
        "upscale_models",
        curated_upscaler_sha256="a" * 64,
    )
