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
"""Prove nonblocking model job coalescing, observable failures, and cleanup."""

from __future__ import annotations

import asyncio

from sugarcubes.model_library.infrastructure.jobs import ModelBackgroundJobs


def test_committed_job_finishes_notification_after_late_cancel() -> None:
    """Keep cancellation from misreporting an already-published model as an abandoned transfer."""

    async def exercise() -> None:
        """Hold the post-commit work while a second cancellation request arrives."""
        jobs = ModelBackgroundJobs()
        committed = asyncio.Event()
        release = asyncio.Event()

        async def operation() -> dict[str, object]:
            """Expose the commit point through its authoritative job state owner."""
            jobs.commit(job.job_id)
            committed.set()
            await release.wait()
            return {"status": "installed"}

        job = jobs.start("download", operation)
        await committed.wait()
        assert jobs.cancel(job.job_id) is job and job.status == "running"
        assert job.to_payload()["cancellable"] is False
        release.set()
        await jobs.wait(job.job_id)
        assert job.status == "completed"
        await jobs.close()

    asyncio.run(exercise())


def test_active_jobs_coalesce_and_publish_completion() -> None:
    """Schedule expensive work once while observers retain the same job identity."""

    async def exercise() -> None:
        """Control completion through events rather than elapsed-time assertions."""
        jobs = ModelBackgroundJobs()
        started = asyncio.Event()
        release = asyncio.Event()
        finished = asyncio.Event()
        calls = 0

        async def operation() -> dict[str, object]:
            """Hold work until the test proves that the request returned first."""
            nonlocal calls
            calls += 1
            started.set()
            await release.wait()
            finished.set()
            return {"revision": 2}

        first = jobs.start("scan", operation)
        assert first.status == "queued"
        assert jobs.start("scan", operation) is first
        await asyncio.wait_for(started.wait(), 2)
        assert first.status == "running"
        release.set()
        await asyncio.wait_for(finished.wait(), 2)
        assert first.status == "completed"
        assert first.result == {"revision": 2}
        assert calls == 1
        assert jobs.get(first.job_id) is first
        await jobs.close()

    asyncio.run(exercise())


def test_failures_are_observable_and_do_not_block_retry() -> None:
    """Publish terminal failures and allow a later explicit request to retry."""

    async def exercise() -> None:
        """Drive the worker outcome with a controlled completion event."""
        jobs = ModelBackgroundJobs()
        finished = asyncio.Event()

        async def failing() -> dict[str, object]:
            """Simulate an external provider failure without a live API call."""
            finished.set()
            raise ValueError("Provider rejected fixture")

        job = jobs.start("provider", failing)
        await asyncio.wait_for(finished.wait(), 2)
        await asyncio.wait_for(jobs.wait(job.job_id), 2)
        assert job.status == "failed"
        assert job.error == "Provider rejected fixture"
        retry = jobs.start("provider", failing)
        assert retry.job_id != job.job_id
        await jobs.close()

    asyncio.run(exercise())


def test_cancellation_before_worker_start_releases_identity() -> None:
    """Avoid retaining a cancelled task that never entered its cleanup block."""

    async def exercise() -> None:
        """Synchronize cleanup with the task lifecycle instead of sleeping."""
        jobs = ModelBackgroundJobs()
        ran = False

        async def operation() -> dict[str, object]:
            """Record whether an already-cancelled worker incorrectly ran."""
            nonlocal ran
            ran = True
            return {}

        first = jobs.start("scan", operation)
        jobs.cancel(first.job_id)
        await jobs.close()
        assert first.status == "cancelled"
        assert not ran
        retry = jobs.start("scan", operation)
        assert retry.job_id != first.job_id
        await jobs.close()

    asyncio.run(exercise())


def test_cancellation_remains_active_until_worker_cleanup_finishes() -> None:
    """Publish terminal cancellation only after reserved files and worker resources are released."""

    async def exercise() -> None:
        """Hold cleanup to prove polling and repeated cancellation cannot race its lifetime."""
        jobs = ModelBackgroundJobs()
        started = asyncio.Event()
        cleanup_started = asyncio.Event()
        cleanup_release = asyncio.Event()
        cleanup_finished = False

        async def operation() -> dict[str, object]:
            """Model an acquisition worker that must release temporary storage asynchronously."""
            nonlocal cleanup_finished
            started.set()
            try:
                await asyncio.Event().wait()
                return {}
            finally:
                cleanup_started.set()
                await cleanup_release.wait()
                cleanup_finished = True

        job = jobs.start("download", operation)
        await started.wait()
        jobs.cancel(job.job_id)
        await cleanup_started.wait()
        try:
            assert job.status == "running"
            assert jobs.active("download") is job
            assert jobs.start("download", operation) is job
            jobs.cancel(job.job_id)
            await asyncio.sleep(0)
            assert job.status == "running" and not cleanup_finished
        finally:
            cleanup_release.set()
            await jobs.wait(job.job_id)
            await jobs.close()
        assert job.status == "cancelled" and cleanup_finished

    asyncio.run(exercise())
