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
"""Validate shared CivitAI artifact evidence for history, discovery, and approved acquisition."""

from __future__ import annotations

import math
import re

from ..domain.provider_releases import ModelArtifactOffer
from ..domain.openmodeldb_catalog import OpenModelDbResource
from ..domain.artifact_eligibility import ProviderFileEvidence, eligible_civitai_file
from ..domain.artifact_transport import approved_civitai_download


def parse_civitai_artifact(
    payload: object, kind: str, *, reviewed_upscaler: OpenModelDbResource | None = None
) -> ModelArtifactOffer | None:
    """Offer only scanned model files with an explicit SHA256 and approved download origin."""
    if not isinstance(payload, dict):
        return None
    metadata = payload.get("metadata")
    file_id, name, hashes = (
        payload.get("id"),
        payload.get("name"),
        payload.get("hashes"),
    )
    size, url = payload.get("sizeKB"), payload.get("downloadUrl")
    if not isinstance(file_id, int) or isinstance(file_id, bool) or file_id <= 0:
        return None
    if not isinstance(name, str) or not isinstance(hashes, dict):
        return None
    digest = hashes.get("SHA256")
    if not isinstance(digest, str) or not re.fullmatch(r"[a-fA-F0-9]{64}", digest):
        return None
    if not eligible_civitai_file(
        ProviderFileEvidence(
            name,
            _text(payload.get("type")),
            _text(metadata.get("format")) if isinstance(metadata, dict) else "",
            _text(payload.get("pickleScanResult")),
            _text(payload.get("virusScanResult")),
            digest.lower(),
        ),
        kind,
        curated_upscaler_sha256=reviewed_upscaler.sha256 if reviewed_upscaler else None,
    ):
        return None
    if reviewed_upscaler is not None:
        size = reviewed_upscaler.size_bytes / 1024
    if (
        not isinstance(size, (int, float))
        or isinstance(size, bool)
        or not math.isfinite(size)
        or not 0 < size <= 128 * 1024 * 1024
    ):
        return None
    if not isinstance(url, str) or not approved_civitai_download(url):
        return None
    size_bytes = round(size * 1024)
    if size_bytes <= 0:
        return None
    return ModelArtifactOffer(str(file_id), name, digest.lower(), size_bytes, url, kind)


def _text(value: object) -> str:
    """Narrow provider claims without interpreting arbitrary response objects as labels."""
    return value if isinstance(value, str) else ""
