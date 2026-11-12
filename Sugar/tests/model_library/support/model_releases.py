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
"""Provide typed, ordered provider release evidence for model-library proofs."""

from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
)


def release(
    version_id: str, base: str = "SDXL 1.0", kind: str = "loras"
) -> ModelProviderRelease:
    """Supply verified provider evidence ordered by the provider, independent of numeric IDs."""
    version = ProviderModelVersion(
        "civitai",
        "10",
        version_id,
        "Fixture",
        version_id,
        base,
        "https://civitai.com/models/10",
        "",
        (),
        None,
        model_type="LORA" if kind == "loras" else "Checkpoint",
    )
    offer = ModelArtifactOffer(
        version_id,
        f"{version_id}.safetensors",
        "a" * 64,
        10,
        f"https://civitai.com/api/download/models/{version_id}",
        kind,
    )
    return ModelProviderRelease(version, "", (offer,))
