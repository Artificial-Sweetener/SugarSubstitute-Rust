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
"""Own coalesced background model work and observable cancellation/failure state."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
import logging
import uuid

_LOGGER = logging.getLogger(__name__)
JobOperation = Callable[[], Awaitable[dict[str, object]]]


@dataclass(slots=True)
class ModelJob:
    """Expose a serializable job state independently of an event-loop task."""

    job_id: str
    operation: str
    status: str = "queued"
    result: dict[str, object] = field(default_factory=dict)
    error: str | None = None
    cancellable: bool = True
    context: dict[str, object] = field(default_factory=dict)
    progress: dict[str, object] = field(default_factory=dict)

    def to_payload(self) -> dict[str, object]:
        """Publish enough state for either frontend to observe completion."""
        return {
            "jobId": self.job_id,
            "operation": self.operation,
            "status": self.status,
            "result": self.result,
            "error": self.error,
            "cancellable": self.cancellable,
            "context": self.context,
            "progress": self.progress,
        }


class ModelBackgroundJobs:
    """Coalesce active requests and retain a bounded completed-job history."""

    def __init__(self) -> None:
        """Prepare one scheduler owned by the running model-library integration."""
        self._jobs: dict[str, ModelJob] = {}
        self._active: dict[str, tuple[ModelJob, asyncio.Task[None]]] = {}

    def start(
        self,
        key: str,
        operation: JobOperation,
        *,
        context: dict[str, object] | None = None,
    ) -> ModelJob:
        """Queue work once per identity and expose its state immediately."""
        active = self._active.get(key)
        if active is not None:
            if not active[1].done():
                return active[0]
            self._release(active[0], active[1])
        job = ModelJob(uuid.uuid4().hex, key)
        job.context = dict(context) if context is not None else {}
        self._jobs[job.job_id] = job
        task = asyncio.create_task(self._run(job, operation))
        self._active[key] = (job, task)
        task.add_done_callback(lambda completed: self._release(job, completed))
        self._trim_completed()
        return job

    def transfer_progress(self, job_id: str, downloaded: int, total: int) -> None:
        """Retain byte progress on the same job observed by browser and desktop clients."""
        job = self.get(job_id)
        if job is not None and job.status == "running":
            job.progress = {"bytesDownloaded": downloaded, "bytesTotal": total}

    def get(self, job_id: str) -> ModelJob | None:
        """Read a retained job without creating or restarting work."""
        job = self._jobs.get(job_id)
        if job is not None:
            active = self._active.get(job.operation)
            if active is not None and active[0] is job and active[1].done():
                self._release(job, active[1])
        return job

    def active(self, operation: str) -> ModelJob | None:
        """Observe coalesced work without creating a second consumer-owned scheduler."""
        current = self._active.get(operation)
        if current is None:
            return None
        job = self.get(current[0].job_id)
        return job if job is not None and job.status in ("queued", "running") else None

    async def wait(self, job_id: str) -> ModelJob | None:
        """Observe terminal worker state without propagating a task's boundary failure."""
        job = self._jobs.get(job_id)
        if job is None:
            return None
        active = self._active.get(job.operation)
        if active is not None and active[0] is job:
            await asyncio.gather(asyncio.shield(active[1]), return_exceptions=True)
            self._release(job, active[1])
        return job

    def cancel(self, job_id: str) -> ModelJob | None:
        """Request cancellation once while the worker retains ownership through cleanup."""
        job = self._jobs.get(job_id)
        if job is not None and job.cancellable:
            active = self._active.get(job.operation)
            if active is not None and active[0] is job and not active[1].cancelling():
                active[1].cancel()
        return job

    def commit(self, job_id: str) -> None:
        """Let completed irreversible publication finish catalog notification despite late cancellation."""
        job = self._jobs.get(job_id)
        if job is None:
            raise ValueError("The committing model job is not retained.")
        job.cancellable = False

    async def close(self) -> None:
        """Release every outstanding task when the host integration stops."""
        tasks = [task for _, task in self._active.values()]
        for job, task in self._active.values():
            if job.cancellable and not task.cancelling():
                task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        self._active.clear()

    async def _run(self, job: ModelJob, operation: JobOperation) -> None:
        """Translate worker outcomes without allowing unexpected failures to disappear."""
        try:
            job.status = "running"
            job.result = await operation()
            job.status = "completed"
        except asyncio.CancelledError:
            job.status = "cancelled"
            raise

    def _release(self, job: ModelJob, task: asyncio.Task[None]) -> None:
        """Release cancelled-before-start tasks without evicting a newer job."""
        if task.cancelled():
            job.status = "cancelled"
        else:
            error = task.exception()
            if error is not None and job.status != "failed":
                _LOGGER.error(
                    "Model library background operation failed",
                    extra={"operation": job.operation, "job_id": job.job_id},
                    exc_info=(type(error), error, error.__traceback__),
                )
                job.status = "failed"
                job.error = (
                    str(error)
                    if isinstance(error, ValueError)
                    else "Model library operation failed. See the server logs for details."
                )
        active = self._active.get(job.operation)
        if active is not None and active[1] is task:
            self._active.pop(job.operation, None)

    def _trim_completed(self) -> None:
        """Keep retained failures and successes bounded without evicting active work."""
        for job_id, job in tuple(self._jobs.items()):
            if len(self._jobs) <= 128:
                break
            if job.status in ("completed", "cancelled", "failed"):
                del self._jobs[job_id]
