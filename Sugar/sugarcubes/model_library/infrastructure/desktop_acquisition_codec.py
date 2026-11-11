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
"""Validate the established desktop acquisition contract at the shared host boundary."""

from __future__ import annotations

from dataclasses import dataclass, field
import re

from ..domain.download_paths import download_relative_path
from ..domain.artifact_transport import approved_civitai_download
from ..domain.artifact_eligibility import ProviderFileEvidence, eligible_civitai_file
from .metadata_codec import normalized_hash
from .provider_credentials import civitai_authorization


@dataclass(frozen=True, slots=True)
class DesktopModelAcquisition:
    """Carry validated acquisition identity and preferences without caller-controlled transport."""

    kind: str
    sha256: str
    relative_path: str
    api_key: str | None = field(repr=False)


def desktop_model_acquisition(value: object) -> DesktopModelAcquisition:
    """Reject unsafe caller evidence before queueing a provider-revalidated shared job."""
    if not isinstance(value, dict):
        raise ValueError("Invalid model download request.")
    kind, digest = _text(value, "kind"), normalized_hash(_text(value, "sha256"))
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,80}", kind):
        raise ValueError("Invalid model category.")
    if not approved_civitai_download(
        _text(value, "downloadUrl")
    ) or not eligible_civitai_file(
        ProviderFileEvidence(
            _text(value, "fileName"),
            _text(value, "fileType"),
            _text(value, "metadataFormat"),
            _text(value, "pickleScanResult").capitalize(),
            _text(value, "virusScanResult").capitalize(),
            digest,
        ),
        kind,
    ):
        raise ValueError(
            "The requested CivitAI artifact does not have safe model evidence."
        )
    tokens: object = value.get("downloadPathTokens", {})
    if not isinstance(tokens, dict) or len(tokens) > 32:
        raise ValueError("Invalid model download path tokens.")
    normalized_tokens: dict[str, str] = {}
    for key, token in tokens.items():
        if not isinstance(key, str) or not isinstance(token, str) or len(token) > 2048:
            raise ValueError("Invalid model download path tokens.")
        normalized_tokens[key] = token
    relative = download_relative_path(
        _text(value, "fileName"),
        _text(value, "downloadPathPattern", "{file_name}"),
        normalized_tokens,
    )
    if not relative.lower().endswith(".safetensors"):
        raise ValueError("The desktop model download requires a SafeTensor filename.")
    credential: object = value.get("apiKey")
    if credential is not None and not isinstance(credential, str):
        raise ValueError("Invalid CivitAI credential.")
    civitai_authorization(lambda: credential)
    return DesktopModelAcquisition(kind, digest, relative, credential)


def _text(row: dict[object, object], key: str, default: str = "") -> str:
    """Bound all untrusted desktop labels before they reach acquisition policy."""
    value = row.get(key, default)
    if not isinstance(value, str) or len(value) > 4096 or "\0" in value:
        raise ValueError("Invalid model download field: " + key)
    return value
