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
"""Model what SugarCubes' own updater is doing, as the Updates entry reports it."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from ..pack_install.ownership import NotOwnedReason
from ..pack_install.startup import StartupOutcome


class UpdatePhase(str, Enum):
    """Name the updater's state; spelled as on the wire."""

    IDLE = "idle"
    CHECKING = "checking"
    UP_TO_DATE = "up-to-date"
    DOWNLOADING = "downloading"
    READY = "ready"
    SCHEDULED = "scheduled"
    FAILED = "failed"
    NOT_MANAGED = "not-managed"


class FailureStage(str, Enum):
    """Name the step that failed; every failure leaves the installed pack as it was."""

    CHECK = "check"
    DOWNLOAD = "download"
    VERIFY = "verify"


@dataclass(frozen=True, slots=True)
class UpdateSnapshot:
    """Describe the updater at one moment, ready to send to the page."""

    current_version: str
    phase: UpdatePhase
    available_version: str | None = None
    failure: FailureStage | None = None
    not_managed: NotOwnedReason | None = None
    last_checked_at_ms: int | None = None
    notice: StartupOutcome | None = None
    revision: int = 1


__all__ = ["FailureStage", "UpdatePhase", "UpdateSnapshot"]
