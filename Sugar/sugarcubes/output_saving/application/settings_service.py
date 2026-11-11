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
"""Coordinate output saving preference validation, migration and example paths."""

from __future__ import annotations

from dataclasses import replace
from typing import Protocol

from ..domain.preference_codec import import_substitute_preferences
from ..domain.preferences import OutputSavingPreferences


class SavingPreferenceRepository(Protocol):
    """Describe the authoritative user settings persistence boundary."""

    def load(self) -> OutputSavingPreferences:
        """Load validated preferences without altering existing bytes."""

    def save(self, preferences: OutputSavingPreferences) -> None:
        """Replace validated preferences atomically."""


class SavingSettingsPaths(Protocol):
    """Validate host-side destinations and render examples without creating files."""

    def validate(self, preferences: OutputSavingPreferences) -> None:
        """Reject unusable roots or escaping rendered paths before saving settings."""

    def preview(self, preferences: OutputSavingPreferences) -> str:
        """Return an example canonical PNG path using Substitute's example facts."""


class OutputSavingSettingsService:
    """Own settings changes independently of browser controls and runtime canvases."""

    def __init__(
        self, repository: SavingPreferenceRepository, paths: SavingSettingsPaths
    ) -> None:
        """Bind persistence and host path boundaries explicitly."""
        self._repository = repository
        self._paths = paths

    def load(self) -> OutputSavingPreferences:
        """Expose the authoritative preferences without silently resetting invalid files."""
        return self._repository.load()

    def save(self, preferences: OutputSavingPreferences) -> OutputSavingPreferences:
        """Validate all host-specific constraints before replacing user settings."""
        self._paths.validate(preferences)
        self._repository.save(preferences)
        return preferences

    def import_substitute(self, value: object) -> OutputSavingPreferences:
        """Migrate saving choices while preserving the user's existing graph-management opt-in."""
        imported = import_substitute_preferences(value)
        current = self._repository.load()
        return self.save(
            replace(
                imported,
                manage_workflow=current.manage_workflow,
                save_previews=current.save_previews,
            )
        )

    def reset(self) -> OutputSavingPreferences:
        """Restore built-in cube saving and disabled ordinary-node management defaults."""
        return self.save(OutputSavingPreferences())

    def preview(self, preferences: OutputSavingPreferences | None = None) -> str:
        """Render proposed or persisted settings without touching output directories."""
        return self._paths.preview(
            preferences if preferences is not None else self.load()
        )
