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
"""Retain OpenModelDB resource identity and presentation for every catalog consumer."""

from __future__ import annotations

from dataclasses import dataclass, field
from types import MappingProxyType
from typing import Mapping

from .provider_models import ProviderModelVersion


@dataclass(frozen=True, slots=True)
class OpenModelDbResource:
    """Describe one exact artifact independently of transport and installation policy."""

    resource_type: str
    size_bytes: int
    sha256: str
    urls: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class OpenModelDbModel:
    """Keep the reference catalog's labels, provenance, and supported resources together."""

    model_id: str
    name: str
    author: str | None
    license_name: str | None
    architecture: str | None
    scale: int | None
    tags: tuple[str, ...]
    description: str | None
    resources: tuple[OpenModelDbResource, ...]
    thumbnail_url: str | None

    @property
    def model_page_url(self) -> str:
        """Return the canonical provider page for this validated identifier."""
        return f"https://openmodeldb.info/models/{self.model_id}"

    @property
    def version_label(self) -> str:
        """Match Substitute's scale and architecture subtitle rather than a catalog date."""
        return " · ".join(
            part
            for part in (
                f"{self.scale}×" if self.scale is not None else None,
                self.architecture.upper() if self.architecture else None,
            )
            if part is not None
        )

    def preferred_resource(self) -> OpenModelDbResource | None:
        """Prefer SafeTensor resources before PyTorch resources in provider order."""
        return next(
            (
                resource
                for resource_type in ("safetensors", "pth")
                for resource in self.resources
                if resource.resource_type == resource_type
            ),
            None,
        )

    def provider_version(self, resource: OpenModelDbResource) -> ProviderModelVersion:
        """Project exact-hash metadata through the same labels used by discovery."""
        if resource not in self.resources:
            raise ValueError("The resource does not belong to this OpenModelDB model.")
        return ProviderModelVersion(
            provider="openmodeldb",
            model_id=self.model_id,
            version_id=resource.sha256,
            title=self.name,
            version=self.version_label,
            base_model=self.architecture or "",
            page_url=self.model_page_url,
            description=self.description or "",
            trained_words=(),
            preview_url=self.thumbnail_url,
        )


@dataclass(frozen=True, slots=True)
class OpenModelDbCatalog:
    """Share immutable catalog evidence with constant-time model and resource lookups."""

    models: tuple[OpenModelDbModel, ...]
    _models_by_id: Mapping[str, OpenModelDbModel] = field(init=False, repr=False)
    _resources_by_hash: Mapping[str, tuple[OpenModelDbModel, OpenModelDbResource]] = (
        field(init=False, repr=False)
    )

    def __post_init__(self) -> None:
        """Preserve provider-order precedence when identical hashes have several listings."""
        models: dict[str, OpenModelDbModel] = {}
        resources: dict[str, tuple[OpenModelDbModel, OpenModelDbResource]] = {}
        for model in self.models:
            models.setdefault(model.model_id, model)
            for resource in model.resources:
                resources.setdefault(resource.sha256, (model, resource))
        object.__setattr__(self, "_models_by_id", MappingProxyType(models))
        object.__setattr__(self, "_resources_by_hash", MappingProxyType(resources))

    def model(self, model_id: str) -> OpenModelDbModel | None:
        """Resolve a stable catalog identifier without scanning all resources."""
        return self._models_by_id.get(model_id)

    def resource_for_sha256(
        self, sha256: str
    ) -> tuple[OpenModelDbModel, OpenModelDbResource] | None:
        """Resolve normalized exact content identity with reference-order precedence."""
        return self._resources_by_hash.get(sha256.casefold())
