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
"""Own provider type, base-family, and file eligibility across model use cases."""

from __future__ import annotations

from dataclasses import dataclass

from .artifact_safety import safe_artifact_name

_CIVITAI_TYPES = {
    "checkpoints": "Checkpoint",
    "diffusion_models": "Checkpoint",
    "loras": "LORA",
    "vae": "VAE",
    "controlnet": "Controlnet",
    "upscale_models": "Upscaler",
    "embeddings": "TextualInversion",
}


@dataclass(frozen=True, slots=True)
class ProviderFileEvidence:
    """Carry normalized provider claims separately from approved transfer evidence."""

    name: str
    file_type: str
    file_format: str
    pickle_scan: str
    virus_scan: str
    sha256: str = ""


def civitai_model_type(kind: str) -> str | None:
    """Return Substitute's exact provider type for an authoritative host artifact role."""
    return _CIVITAI_TYPES.get(kind)


def civitai_model_kind(model_type: str) -> str | None:
    """Normalize declared provider types without inferring aliases from filenames or folders."""
    return next(
        (kind for kind, declared in _CIVITAI_TYPES.items() if declared == model_type),
        None,
    )


def matches_civitai_kind(model_type: str, kind: str) -> bool:
    """Require exact provider model-type evidence before adapting a host destination."""
    expected = civitai_model_type(kind)
    return expected is not None and expected == model_type


def compatible_civitai_artifact_role(
    model_type: str, source_kind: str, target_kind: str
) -> bool:
    """Permit host-role adaptation only when both roles share the declared provider type."""
    return matches_civitai_kind(model_type, source_kind) and matches_civitai_kind(
        model_type, target_kind
    )


def same_base_family(candidate: str, current: str) -> bool:
    """Require a known exact base rather than inferring compatibility from a family name."""
    return (
        bool(current.strip())
        and candidate.strip().casefold() == current.strip().casefold()
    )


def eligible_civitai_file(
    evidence: ProviderFileEvidence,
    kind: str,
    *,
    curated_upscaler_sha256: str | None = None,
) -> bool:
    """Apply the reference's normal SafeTensor and exact curated-upscaler policies.

    Curated upscalers are identified by the reviewed OpenModelDB artifact hash.
    They may use PyTorch weights, whose pickle scan must be clean. That exception
    cannot authorize another model role or an arbitrary unreviewed artifact.
    """
    if civitai_model_type(kind) is None or not safe_artifact_name(evidence.name):
        return False
    suffix = evidence.name.rpartition(".")[2].casefold()
    if curated_upscaler_sha256 is not None:
        return (
            kind == "upscale_models"
            and len(curated_upscaler_sha256) == 64
            and all(
                character in "0123456789abcdef" for character in curated_upscaler_sha256
            )
            and evidence.sha256.casefold() == curated_upscaler_sha256
            and suffix in {"pth", "pt", "safetensors"}
            and evidence.virus_scan.casefold() == "success"
            and (
                suffix == "safetensors" or evidence.pickle_scan.casefold() == "success"
            )
        )
    return (
        evidence.file_type.casefold() == "model"
        and suffix == "safetensors"
        and evidence.file_format.casefold() == "safetensor"
        and evidence.pickle_scan == "Success"
        and evidence.virus_scan == "Success"
    )
