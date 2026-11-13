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
"""Provide deterministic remote responses for real CivitAI discovery adapter tests."""

from __future__ import annotations

import json
from urllib.parse import parse_qs, urlsplit

from .provider_transport import TransportResponse, TransportSession


def discovery_model(
    model_id: int, version_id: int, digest: str, *, base: str = "Illustrious"
) -> dict[str, object]:
    """Return the complete provider contract with literal captions and a scan-clean artifact."""
    return {
        "id": model_id,
        "name": f"Model {model_id}",
        "type": "Checkpoint",
        "nsfw": False,
        "creator": {"username": "<literal creator>"},
        "modelVersions": [
            {
                "id": version_id,
                "name": f"Version {version_id}",
                "baseModel": base,
                "availability": "Public",
                "files": [discovery_file(digest)],
                "images": [discovery_image(version_id)],
            }
        ],
    }


def discovery_file(digest: str) -> dict[str, object]:
    """Keep one verified file identity independent of the response's model role."""
    return {
        "id": 100,
        "name": "model.safetensors",
        "primary": True,
        "type": "Model",
        "metadata": {"format": "SafeTensor"},
        "hashes": {"SHA256": digest},
        "pickleScanResult": "Success",
        "virusScanResult": "Success",
        "sizeKB": 12,
        "downloadUrl": "https://civitai.com/api/download/models/100",
    }


def discovery_image(image_id: int) -> dict[str, object]:
    """Supply the large portrait required by the reference recommendation cards."""
    return {
        "id": image_id,
        "url": f"https://image.civitai.com/id{image_id}/original=true/preview.jpeg",
        "width": 768,
        "height": 1024,
        "nsfwLevel": 1,
        "type": "image",
    }


def public_access(*, requires_key: bool = False) -> dict[str, object]:
    """Represent explicit anonymous access rather than assuming it from a download link."""
    return {
        "availability": "Public",
        "requireAuth": requires_key,
        "checkPermission": False,
    }


class DiscoveryTransportSession(TransportSession):
    """Replace only HTTP response bytes while retaining the production request and parsing flow."""

    def __init__(self, routes: dict[str, object]) -> None:
        """Map reviewed API paths and relevant pagination/gallery identities to provider responses."""
        super().__init__(TransportResponse(200, b"{}"))
        self.routes = routes

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Fail unexpected requests instead of silently allowing an accidental network path."""
        parsed = urlsplit(url)
        query = parse_qs(parsed.query)
        key = parsed.path
        if "page" in query:
            key += ":page=" + query["page"][0]
        if "modelVersionId" in query:
            key += ":version=" + query["modelVersionId"][0]
        payload = self.routes[key]
        self.response = (
            payload
            if isinstance(payload, TransportResponse)
            else TransportResponse(200, json.dumps(payload).encode())
        )
        return super().get(url, allow_redirects=allow_redirects, headers=headers)
