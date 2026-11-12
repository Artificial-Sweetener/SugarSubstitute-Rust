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
"""Run SugarCubes' own update checks in the background and record the user's choice.

The page asks for a check only when no host owns pack updates (the host
description decides; inside Substitute the host updates SugarCubes during its
splash). Checks never block a request: one background job at a time, coalesced,
and skipped while a recent check is still fresh unless the user asks again.
Installing is never done here: the user's choice is recorded, and the files are
replaced the next time ComfyUI starts, before SugarCubes loads, so the running
backend is never restarted or changed mid-session.
"""

from __future__ import annotations

import logging
import threading
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from pathlib import Path
from typing import Protocol

from ..pack_install.errors import PackInstallError
from ..pack_install.file_lock import exclusive_update_lock
from ..pack_install.ownership import NotOwnedPack
from ..pack_install.staging import StagedStore
from ..pack_install.startup import (
    clear_outcome,
    accept_backend_start,
    read_outcome,
    report_page_load,
    updates_root,
)
from .release_stager import NotManaged, ReleaseStager, Staged, StageFailed, StageResult
from .update_state import FailureStage, UpdatePhase, UpdateSnapshot

_logger = logging.getLogger("sugarcubes.self_update")
RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000
Clock = Callable[[], int]
Listener = Callable[[UpdateSnapshot], None]


class SelfUpdateRejected(RuntimeError):
    """Report a request the updater cannot accept in its current state."""


class BackgroundRunner(Protocol):
    """Run one updater job at a time away from the request thread."""

    def submit(self, job: Callable[[], None]) -> None:
        """Start one job."""

    def close(self) -> None:
        """Stop accepting jobs and wait for the running one to end."""


class ThreadRunner:
    """Run jobs on one owned worker thread with bounded shutdown."""

    def __init__(self) -> None:
        """Create the single worker the updater owns."""

        self._executor = ThreadPoolExecutor(
            max_workers=1, thread_name_prefix="sugarcubes-self-update"
        )

    def submit(self, job: Callable[[], None]) -> None:
        """Queue one job on the worker."""

        self._executor.submit(job)

    def close(self) -> None:
        """Stop the worker; the job's own cancellation check bounds the wait."""

        self._executor.shutdown(wait=True, cancel_futures=True)


class SelfUpdateService:
    """Own the updater state the Updates entry shows, and its transitions."""

    def __init__(
        self,
        *,
        pack_folder: Path,
        running_version: str,
        stager: ReleaseStager,
        clock: Clock,
        runner: BackgroundRunner,
    ) -> None:
        """Load the state left on disk by earlier checks and starts."""

        self._pack_folder = pack_folder
        self._running_version = running_version
        self._stager = stager
        self._clock = clock
        self._runner = runner
        self._root = updates_root(pack_folder)
        self._store = StagedStore(self._root)
        self._guard = threading.Lock()
        self._closing = threading.Event()
        self._job_running = False
        self._listeners: list[Listener] = []
        self._snapshot = self._initial_snapshot()

    def subscribe(self, listener: Listener) -> Callable[[], None]:
        """Observe every state change; listeners may be called from the worker."""

        with self._guard:
            self._listeners.append(listener)

        def unsubscribe() -> None:
            with self._guard:
                if listener in self._listeners:
                    self._listeners.remove(listener)

        return unsubscribe

    def snapshot(self) -> UpdateSnapshot:
        """Return the current state."""

        with self._guard:
            return self._snapshot

    def accept_backend_start(self, *, nodes_registered: bool) -> None:
        """Accept a just-installed version once ComfyUI finished a full start.

        Called when ComfyUI's server is listening. `nodes_registered` says
        whether ComfyUI registered every SugarCubes node class, which proves
        the pack's whole import completed; without it nothing is accepted and
        the next start restores the previous version.
        """

        if not nodes_registered:
            _logger.warning(
                "SugarCubes update event=start_incomplete version=%s",
                self._running_version,
            )
            return
        try:
            outcome = accept_backend_start(self._pack_folder, self._running_version)
        except (OSError, PackInstallError) as exc:
            _logger.warning("SugarCubes update event=accept_failed cause=%s", exc)
            return
        if outcome is not None:
            with self._guard:
                self._commit(replace(self._snapshot, notice=outcome))
            self._notify()

    def report_page_load(self, *, loaded: bool) -> None:
        """Settle a started version by its first page load."""

        try:
            report_page_load(self._pack_folder, self._running_version, loaded=loaded)
        except (OSError, PackInstallError) as exc:
            _logger.warning("SugarCubes update event=page_report_failed cause=%s", exc)

    def request_check(self, *, force: bool) -> UpdateSnapshot:
        """Start a background check unless one runs or a recent one is fresh."""

        with self._guard:
            snapshot = self._snapshot
            fresh = (
                snapshot.last_checked_at_ms is not None
                and self._clock() - snapshot.last_checked_at_ms < RECHECK_INTERVAL_MS
            )
            if (
                self._job_running
                or self._closing.is_set()
                or snapshot.phase in {UpdatePhase.NOT_MANAGED, UpdatePhase.SCHEDULED}
                and not force
                or fresh
                and not force
            ):
                return snapshot
            self._job_running = True
            started = self._commit(
                replace(snapshot, phase=UpdatePhase.CHECKING, failure=None)
            )
        self._notify()
        self._runner.submit(self._run_check)
        return started

    def schedule_install(self, version: str) -> UpdateSnapshot:
        """Record the user's choice to install the ready release at the next start."""

        with self._guard:
            if self._job_running or self._snapshot.phase is not UpdatePhase.READY:
                raise SelfUpdateRejected("There is no update ready to install.")
            if self._snapshot.available_version != version:
                raise SelfUpdateRejected("A different update is ready now.")
            try:
                with exclusive_update_lock(self._root):
                    release = self._store.load()
                    if release is None or release.version != version:
                        raise SelfUpdateRejected("The update is no longer ready.")
                    self._store.verify(release)
                    self._store.schedule(release)
            except PackInstallError as exc:
                raise SelfUpdateRejected("The update could not be prepared.") from exc
            scheduled = self._commit(
                replace(self._snapshot, phase=UpdatePhase.SCHEDULED)
            )
            _logger.info(
                "SugarCubes update event=install_scheduled version=%s", version
            )
        self._notify()
        return scheduled

    def cancel_install(self) -> UpdateSnapshot:
        """Withdraw the user's choice; the release stays ready."""

        with self._guard:
            if self._snapshot.phase is not UpdatePhase.SCHEDULED:
                return self._snapshot
            try:
                with exclusive_update_lock(self._root):
                    self._store.unschedule()
            except PackInstallError as exc:
                raise SelfUpdateRejected(
                    "The choice could not be changed now."
                ) from exc
            ready = self._commit(replace(self._snapshot, phase=UpdatePhase.READY))
        self._notify()
        return ready

    def dismiss_notice(self) -> UpdateSnapshot:
        """Forget the report of what the last start did."""

        with self._guard:
            clear_outcome(self._root)
            dismissed = self._commit(replace(self._snapshot, notice=None))
        self._notify()
        return dismissed

    def close(self) -> None:
        """Cancel the running check and stop the worker."""

        self._closing.set()
        self._runner.close()

    def _run_check(self) -> None:
        """Run one check on the worker and publish its result."""

        result: StageResult
        try:
            result = self._stager.check_and_stage(
                installed_version=self._running_version,
                cancelled=self._closing.is_set,
                on_downloading=self._publish_downloading,
            )
        except Exception:
            # The job supervisor: an unexpected fault must still end the check visibly.
            _logger.exception("SugarCubes update event=check_crashed")
            result = StageFailed(
                self._running_version, FailureStage.CHECK, "unexpected"
            )
        with self._guard:
            self._job_running = False
            self._commit(self._result_snapshot(result))
        self._notify()

    def _publish_downloading(self, version: str) -> None:
        """Show that a newer release is being downloaded."""

        with self._guard:
            self._commit(
                replace(
                    self._snapshot,
                    phase=UpdatePhase.DOWNLOADING,
                    available_version=version,
                )
            )
        self._notify()

    def _commit(self, snapshot: UpdateSnapshot) -> UpdateSnapshot:
        """Store a new state with the next revision; call with the guard held."""

        self._snapshot = replace(snapshot, revision=self._snapshot.revision + 1)
        return self._snapshot

    def _notify(self) -> None:
        """Hand the current state to every listener, outside the guard."""

        with self._guard:
            snapshot = self._snapshot
            listeners = tuple(self._listeners)
        for listener in listeners:
            listener(snapshot)

    def _result_snapshot(self, result: StageResult) -> UpdateSnapshot:
        """Translate one attempt into the state the page shows."""

        now = self._clock()
        base = replace(
            self._snapshot,
            current_version=result.current_version,
            available_version=None,
            failure=None,
            not_managed=None,
            unchecked_sources=tuple(kind.value for kind in result.unchecked_sources),
        )
        if isinstance(result, NotManaged):
            return replace(
                base,
                phase=UpdatePhase.NOT_MANAGED,
                not_managed=result.reason,
                last_checked_at_ms=now,
            )
        if isinstance(result, StageFailed):
            return replace(base, phase=UpdatePhase.FAILED, failure=result.stage)
        if isinstance(result, Staged):
            return replace(
                base,
                phase=UpdatePhase.READY,
                available_version=result.version,
                last_checked_at_ms=now,
            )
        return replace(base, phase=UpdatePhase.UP_TO_DATE, last_checked_at_ms=now)

    def _initial_snapshot(self) -> UpdateSnapshot:
        """Rebuild the state from the folder, the staging area and the last start."""

        snapshot = UpdateSnapshot(
            current_version=self._running_version, phase=UpdatePhase.IDLE
        )
        ownership = self._stager.ownership()
        if isinstance(ownership, NotOwnedPack):
            return replace(
                snapshot, phase=UpdatePhase.NOT_MANAGED, not_managed=ownership.reason
            )
        try:
            snapshot = replace(snapshot, notice=read_outcome(self._root))
            release = self._store.load()
        except (OSError, PackInstallError) as exc:
            _logger.warning("SugarCubes update event=state_unreadable cause=%s", exc)
            return snapshot
        if release is None or release.from_version != self._running_version:
            return snapshot
        phase = (
            UpdatePhase.SCHEDULED
            if self._store.scheduled_for(release)
            else UpdatePhase.READY
        )
        return replace(snapshot, phase=phase, available_version=release.version)


__all__ = [
    "BackgroundRunner",
    "RECHECK_INTERVAL_MS",
    "SelfUpdateRejected",
    "SelfUpdateService",
    "ThreadRunner",
]
