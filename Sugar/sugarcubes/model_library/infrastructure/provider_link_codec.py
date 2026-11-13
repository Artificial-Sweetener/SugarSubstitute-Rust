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
"""Validate provider associations before persistent storage or navigation projection."""

from __future__ import annotations

import json
from urllib.parse import urlsplit

from ..domain.provider_links import ModelProviderLink


def encode_provider_link(link: ModelProviderLink) -> str:
    """Serialize one validated association independently of presentation metadata."""
    payload = json.dumps(link.to_payload(), ensure_ascii=False, sort_keys=True)
    decode_provider_link(payload)
    return payload


def decode_provider_link(payload: str) -> ModelProviderLink:
    """Narrow a stored association through the same owner as metadata input."""
    return provider_link_value(json.loads(payload))


def provider_link_value(value: object) -> ModelProviderLink:
    """Reject executable, credential-bearing and foreign provider page origins."""
    if not isinstance(value, dict):
        raise ValueError("Model provider link must be an object.")
    provider = _string(value.get("providerId"))
    hosts = {
        "civitai": "civitai.com",
        "openmodeldb": "openmodeldb.info",
        "huggingface": "huggingface.co",
    }
    page = _string(value.get("modelPageUrl"))
    parsed = urlsplit(page)
    if (
        provider.casefold() not in hosts
        or parsed.scheme != "https"
        or parsed.hostname != hosts[provider.casefold()]
        or parsed.username
        or parsed.password
        or parsed.port not in (None, 443)
        or len(page) > 2048
    ):
        raise ValueError("Invalid model provider page association.")
    return ModelProviderLink(
        provider,
        _string(value.get("providerName")),
        _string(value.get("modelId")),
        _string(value.get("versionId")),
        page,
    )


def _string(value: object) -> str:
    """Narrow association text without silently coercing stored data."""
    if not isinstance(value, str):
        raise ValueError("Expected model provider association text.")
    return value
