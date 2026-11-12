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
"""Retain ordered provider associations for the same verified model artifact."""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Iterable


@dataclass(frozen=True, slots=True)
class ModelProviderLink:
    """Identify one provider page independently from the chosen presentation source."""

    provider_id: str
    provider_name: str
    model_id: str
    version_id: str
    page_url: str

    def to_payload(self) -> dict[str, str]:
        """Expose host-neutral associations without credentials or transport addresses."""
        return {
            "providerId": self.provider_id,
            "providerName": self.provider_name,
            "modelId": self.model_id,
            "versionId": self.version_id,
            "modelPageUrl": self.page_url,
        }


def unique_provider_links(
    links: Iterable[ModelProviderLink],
) -> tuple[ModelProviderLink, ...]:
    """Keep the first match for each provider, preserving Substitute's precedence."""
    unique: dict[str, ModelProviderLink] = {}
    for link in links:
        unique.setdefault(link.provider_id.casefold(), link)
    return tuple(unique.values())


def primary_provider_link(
    provider: str, model_id: str, version_id: str, page_url: str
) -> tuple[ModelProviderLink, ...]:
    """Project a persisted primary association into the public ordered-link contract."""
    names = {
        "civitai": "CivitAI",
        "openmodeldb": "OpenModelDB",
        "huggingface": "Hugging Face",
    }
    name = names.get(provider.casefold())
    return (
        (ModelProviderLink(provider, name, model_id, version_id, page_url),)
        if name is not None and page_url
        else ()
    )
