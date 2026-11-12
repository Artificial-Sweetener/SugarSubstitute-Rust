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
"""Say in plain words what the last ComfyUI start did with a chosen update.

The Updates entry owns the wording of every state; this sentence is the cause it
shows in the report of a start that did not end on the new version. It names
what happened and what the user can expect, and never mentions internals.
"""

from __future__ import annotations

from ..pack_install.startup import StartupOutcome, StartupOutcomeKind


def describe_notice(outcome: StartupOutcome) -> str:
    """Return the sentence that reports what the last start did."""

    if outcome.kind is StartupOutcomeKind.INSTALLED:
        return f"SugarCubes was updated to {outcome.version}."
    if outcome.kind is StartupOutcomeKind.RESTORED and outcome.reason == "interrupted":
        return (
            f"Installing SugarCubes {outcome.version} was interrupted, so the "
            f"previous version ({outcome.from_version}) was put back. Nothing else changed."
        )
    if outcome.kind is StartupOutcomeKind.RESTORED:
        return (
            f"SugarCubes {outcome.version} didn't start properly, so the previous "
            f"version ({outcome.from_version}) was put back. Nothing else changed."
        )
    return (
        f"SugarCubes {outcome.version} wasn't installed, and nothing was changed. "
        "SugarCubes will offer it again after its next check."
    )


__all__ = ["describe_notice"]
