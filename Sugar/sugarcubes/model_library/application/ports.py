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
"""Declare the authoritative model registry and persistent catalog boundaries."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol

from ..domain.models import (
    ModelCatalogEntry,
    ModelEvidence,
    ModelMetadata,
    CatalogSnapshot,
    ThumbnailReference,
)
from ..domain.provider_metadata import ProviderMetadataRecord


class ModelRoots(Protocol):
    """Resolve model choices using the running host's actual folder registry."""

    def list_files(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelEvidence, ...]:
        """Return stat evidence for approved, currently selectable model files."""

    def resolve_file(self, kind: str, value: str) -> str | None:
        """Return an approved physical path for an exact host choice."""

    def storage_root(self) -> str:
        """Return the active configured model folder for shared library storage."""

    def install_root(self, kind: str) -> str:
        """Return the host-approved target folder for a known model category."""

    def configured_folders(self) -> dict[str, tuple[str, ...]]:
        """Return approved category roots for stat-only change observation."""


class ModelCatalogRepository(Protocol):
    """Persist file evidence separately from provider and user-owned metadata."""

    def reconcile(
        self, entries: Sequence[ModelEvidence], kinds: Sequence[str] | None
    ) -> int:
        """Replace a scan's file evidence and invalidate changed fingerprints."""

    def snapshot(self, kinds: Sequence[str] | None = None) -> CatalogSnapshot:
        """Read rows and their revision from one consistent transaction."""

    def list_entries(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelCatalogEntry, ...]:
        """Read indexed catalog records without accessing model weights."""

    def revision(self) -> int:
        """Return a monotonic token for catalog consumers."""

    def changes_since(self, held: int) -> tuple[int, frozenset[str] | None]:
        """Return the current revision and the models changed after a held one.

        The models are None when the change cannot be named model by model.
        """

    def find_entries(self, identities: Sequence[str]) -> tuple[ModelCatalogEntry, ...]:
        """Read the named models as they are now, leaving out any that no longer exist."""

    def has_content(self, kind: str, sha256: str) -> bool:
        """Say whether a file of this kind with exactly this content is in the catalog."""

    def find_entry(self, location_id: str) -> ModelCatalogEntry | None:
        """Resolve a catalog identity without accepting caller-provided filesystem paths."""

    def save_thumbnail_choice(
        self, evidence: ModelEvidence, thumbnails: tuple[ThumbnailReference, ...]
    ) -> bool:
        """Keep a deliberate image selection above automatic provider and local previews."""

    def save_fingerprint(self, evidence: ModelEvidence, sha256: str) -> bool:
        """Attach a hash only if its file evidence still matches the catalog."""

    def save_user_metadata(
        self,
        sha256: str,
        metadata: ModelMetadata,
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Publish a deliberate user choice independently of every provider match."""

    def provider_records(self, sha256: str) -> tuple[ProviderMetadataRecord, ...]:
        """Read every stored provider match for a file's content, in presentation order."""

    def save_provider_metadata(
        self,
        sha256: str,
        records: tuple[ProviderMetadataRecord, ...],
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Persist every exact provider match and its image ownership independently."""
