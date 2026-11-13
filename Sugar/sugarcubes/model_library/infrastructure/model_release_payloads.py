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
"""Project authoritative version families for browser and sibling-extension presentation."""

from __future__ import annotations

from ..application.ports import ModelCatalogRepository
from ..application.release_service import ModelReleaseFamily
from ..domain.provider_releases import ModelProviderRelease


def model_release_payload(
    release: ModelProviderRelease,
    model_id: str,
    *,
    on_disk: bool = False,
    in_use: bool = False,
    installed_file_name: str | None = None,
) -> dict[str, object]:
    """Expose exact choices and shared preview endpoints while retaining transport URLs privately."""
    return {
        **release.to_payload(),
        "provider": release.version.provider,
        "providerModelId": release.version.model_id,
        "modelName": release.version.title,
        "onDisk": on_disk,
        "installedFileName": installed_file_name,
        "inUse": in_use,
        "previewUrl": (
            f"/sugarcubes/v1/models/{model_id}/versions/{release.version.version_id}/preview"
            if release.version.preview_url
            else None
        ),
    }


def model_release_history_payload(
    repository: ModelCatalogRepository, family: ModelReleaseFamily, model_id: str
) -> dict[str, object]:
    """Resolve one verified chronology and annotate its exact installed artifacts consistently."""
    metadata = family.current.metadata
    installed = {
        (entry.evidence.kind, entry.sha256): entry.evidence.relative_path
        for entry in repository.list_entries()
        if entry.sha256 is not None
    }
    if family.current.sha256 is not None:
        installed[(family.current.evidence.kind, family.current.sha256)] = (
            family.current.evidence.relative_path
        )

    def installed_filename(release: ModelProviderRelease) -> str | None:
        """Match any offered artifact to its exact indexed local filename."""
        if any(
            offer.kind == family.current.evidence.kind
            and offer.sha256 == family.current.sha256
            for offer in release.offers
        ):
            return family.current.evidence.relative_path
        return next(
            (
                installed[(offer.kind, offer.sha256)]
                for offer in release.offers
                if (offer.kind, offer.sha256) in installed
            ),
            None,
        )

    return {
        "schemaVersion": 1,
        "versions": [
            model_release_payload(
                release,
                model_id,
                on_disk=installed_filename(release) is not None,
                installed_file_name=installed_filename(release),
                in_use=metadata is not None
                and release.version.version_id == metadata.provider_version_id,
            )
            for release in family.releases
        ],
    }
