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
"""Keep exact Comfy selections separate from enriched model presentation."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
from typing import TYPE_CHECKING
from .architecture import ModelArchitecture
from .provider_links import (
    ModelProviderLink,
    primary_provider_link,
    unique_provider_links,
)

if TYPE_CHECKING:
    from .thumbnail_sources import ThumbnailSourceOrigin


def model_location_id(kind: str, root_id: str, relative_path: str) -> str:
    """Identify a physical catalog entry without conflating duplicate names."""
    identity = "\0".join((kind, root_id, relative_path.replace("\\", "/")))
    return hashlib.sha256(identity.encode("utf-8")).hexdigest()


@dataclass(frozen=True, slots=True)
class ModelEvidence:
    """Carry filesystem observations without loading model weights."""

    kind: str
    value: str
    root_id: str
    relative_path: str
    size_bytes: int
    modified_ns: int

    @property
    def location_id(self) -> str:
        """Return the stable identity for this physical model location."""
        return model_location_id(self.kind, self.root_id, self.relative_path)

    @property
    def file_title(self) -> str:
        """Use the filename stem when exact provider or local labels are unavailable."""
        return (
            self.relative_path.replace("\\", "/").rsplit("/", 1)[-1].rsplit(".", 1)[0]
        )


@dataclass(frozen=True, slots=True)
class ThumbnailReference:
    """Describe prepared immutable bytes for either frontend to request."""

    asset_id: str
    role: str
    width: int
    height: int
    content_type: str

    def to_payload(self) -> dict[str, object]:
        """Expose dimensions before image decoding so tile geometry is stable."""
        return {
            "assetId": self.asset_id,
            "role": self.role,
            "width": self.width,
            "height": self.height,
            "contentType": self.content_type,
            "url": f"/sugarcubes/v1/models/assets/{self.asset_id}",
        }


@dataclass(frozen=True, slots=True)
class ModelMetadata:
    """Keep provider presentation separate from exact file-selection evidence."""

    title: str
    version: str = ""
    base_model: str = ""
    provider: str = ""
    provider_url: str = ""
    description: str = ""
    trained_words: tuple[str, ...] = ()
    thumbnails: tuple[ThumbnailReference, ...] = ()
    provider_model_id: str = ""
    provider_version_id: str = ""
    provider_links: tuple[ModelProviderLink, ...] = ()
    tags: tuple[str, ...] = ()
    model_type: str = ""
    provider_nsfw: bool | None = None
    provider_nsfw_level: int | None = None

    def links(self) -> tuple[ModelProviderLink, ...]:
        """Preserve explicit ordered associations and the original primary provider."""
        return unique_provider_links(
            (
                *self.provider_links,
                *primary_provider_link(
                    self.provider,
                    self.provider_model_id,
                    self.provider_version_id,
                    self.provider_url,
                ),
            )
        )


@dataclass(frozen=True, slots=True)
class ModelCatalogEntry:
    """Project optional enrichment onto an authoritative Comfy choice."""

    evidence: ModelEvidence
    sha256: str | None = None
    metadata: ModelMetadata | None = None
    thumbnail_origin: ThumbnailSourceOrigin | None = None
    architecture: ModelArchitecture | None = None
    nsfw_override: bool | None = None

    def to_payload(self) -> dict[str, object]:
        """Retain literal values and missing metadata in the public catalog."""
        evidence = self.evidence
        metadata = self.metadata
        path = evidence.relative_path.replace("\\", "/")
        title = metadata.title if metadata else evidence.file_title
        return {
            "id": evidence.location_id,
            "kind": evidence.kind,
            "value": evidence.value,
            "rootId": evidence.root_id,
            "relativePath": path,
            "folder": path.rsplit("/", 1)[0] if "/" in path else "",
            "sizeBytes": evidence.size_bytes,
            "modifiedNs": str(evidence.modified_ns),
            "sha256": self.sha256,
            "title": title,
            "version": metadata.version if metadata else "",
            "baseModel": metadata.base_model if metadata else "",
            "architecture": (
                self.architecture.to_payload() if self.architecture else None
            ),
            "provider": metadata.provider if metadata else "",
            "providerUrl": metadata.provider_url if metadata else "",
            "providerModelId": metadata.provider_model_id if metadata else "",
            "providerVersionId": metadata.provider_version_id if metadata else "",
            "providerLinks": (
                [link.to_payload() for link in metadata.links()] if metadata else []
            ),
            "description": metadata.description if metadata else "",
            "trainedWords": list(metadata.trained_words) if metadata else [],
            "tags": list(metadata.tags) if metadata else [],
            "thumbnails": (
                [item.to_payload() for item in metadata.thumbnails] if metadata else []
            ),
        }


@dataclass(frozen=True, slots=True)
class CatalogSnapshot:
    """Pair catalog rows with the revision from the same committed transaction."""

    revision: int
    entries: tuple[ModelCatalogEntry, ...]
