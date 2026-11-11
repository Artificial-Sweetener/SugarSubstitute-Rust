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
"""Share observable cache maintenance jobs across HTTP and host consumers."""

from __future__ import annotations

from ..application.provider_cache_service import ModelProviderCacheService
from ..domain.provider_cache import provider_cache_action
from .jobs import ModelBackgroundJobs, ModelJob


class ModelProviderCacheJobs:
    """Retain non-cancellable publication once the exclusive maintenance transition commits."""

    def __init__(
        self, service: ModelProviderCacheService, jobs: ModelBackgroundJobs
    ) -> None:
        """Reuse the runtime's single job lifecycle rather than creating a cache scheduler."""
        self._service = service
        self._jobs = jobs

    def start(self, value: object) -> ModelJob:
        """Validate and queue distinct actions so competing requests cannot silently coalesce."""
        action = provider_cache_action(value)

        async def operation() -> dict[str, object]:
            """Publish the exact operation result through the job's shared observation surface."""
            result = await self._service.execute(
                action, lambda: self._jobs.commit(job.job_id)
            )
            return result.to_payload()

        job = self._jobs.start(f"model-cache:civitai:{action}", operation)
        return job
