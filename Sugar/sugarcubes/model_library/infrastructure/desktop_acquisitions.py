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
"""Run existing desktop acquisitions through the shared installation and catalog owners."""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Callable

from ..application.ports import ModelRoots
from ..application.requested_artifacts import RequestedModelArtifacts
from ..application.organized_downloads import DestinationInstaller
from .desktop_acquisition_codec import desktop_model_acquisition
from .jobs import ModelBackgroundJobs, ModelJob
from .artifact_publication import ModelArtifactPublication
from ..domain.provider_releases import InstalledModelArtifact
from .provider_credentials import model_request_credentials


@dataclass(frozen=True, slots=True)
class DesktopModelAcquisitions:
    """Keep legacy wire translation out of provider, download, and persistence policies."""

    artifacts: RequestedModelArtifacts
    installer: DestinationInstaller
    roots: ModelRoots
    publication: ModelArtifactPublication
    jobs: ModelBackgroundJobs
    authorize_download: Callable[[], None]

    def start(self, value: object) -> ModelJob:
        """Validate a desktop recipe and queue one shared cancellable acquisition."""
        request = desktop_model_acquisition(value)
        self.authorize_download()
        self.roots.install_root(request.kind)

        async def operation() -> dict[str, object]:
            """Commit installed bytes, verified identity, and notifications in that order."""
            with model_request_credentials(request.api_key):
                offer = await self.artifacts.offer(request.sha256, request.kind)
                filename = await self.installer.install_to(
                    offer,
                    request.relative_path,
                    lambda downloaded, total: self.jobs.transfer_progress(
                        job.job_id, downloaded, total
                    ),
                )
            self.jobs.commit(job.job_id)
            return await self.publication.publish(
                InstalledModelArtifact(filename, offer)
            )

        job = self.jobs.start(
            "desktop-acquire:"
            + request.kind
            + ":"
            + request.sha256
            + ":"
            + request.relative_path,
            operation,
            context={
                "kind": request.kind,
                "sha256": request.sha256,
                "relativePath": request.relative_path,
            },
        )
        return job
