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
"""Encode the updater state as the JSON the Updates entry reads (schema 1).

Shape: `schema`, `currentVersion`, `state` (an `UpdatePhase` value),
`availableVersion`, `failure` (`check` | `download` | `verify` | null),
`notManaged` (a `NotOwnedReason` value or null), `lastCheckedAt` (milliseconds
since the Unix epoch or null), `revision` (increases with every change, so the page
keeps only the newest state), and `notice` (`{kind, reason, version, fromVersion,
message}` reporting what the last start did, or null). The page owns the wording of
every state; `notice.message` is the plain cause sentence of that report.
"""

from __future__ import annotations

from .update_messages import describe_notice
from .update_state import UpdateSnapshot

WIRE_SCHEMA = 1


def snapshot_to_wire(snapshot: UpdateSnapshot) -> dict[str, object]:
    """Return the JSON object for one snapshot."""

    notice = snapshot.notice
    return {
        "schema": WIRE_SCHEMA,
        "currentVersion": snapshot.current_version,
        "state": snapshot.phase.value,
        "availableVersion": snapshot.available_version,
        "failure": snapshot.failure.value if snapshot.failure else None,
        "notManaged": snapshot.not_managed.value if snapshot.not_managed else None,
        "lastCheckedAt": snapshot.last_checked_at_ms,
        "revision": snapshot.revision,
        "notice": (
            None
            if notice is None or notice.reason == "awaiting-start"
            else {
                "kind": notice.kind.value,
                "reason": notice.reason,
                "version": notice.version,
                "fromVersion": notice.from_version,
                "message": describe_notice(notice),
            }
        ),
    }


__all__ = ["WIRE_SCHEMA", "snapshot_to_wire"]
