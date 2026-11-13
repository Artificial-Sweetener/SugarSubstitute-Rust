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
"""Exercise real preference files and atomic replacement failure boundaries."""

from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.output_saving.domain.preferences import OutputSavingPreferences
from sugarcubes.output_saving.infrastructure.preference_repository import (
    FileOutputSavingPreferences,
)


def test_round_trip_retains_management_and_inactive_companion_values(
    tmp_path: Path,
) -> None:
    """Store one complete policy without enabling saving scopes through default fallback."""
    repository = FileOutputSavingPreferences(tmp_path / "settings" / "output.json")
    defaults = repository.load()
    selected = replace(defaults, manage_workflow=True, save_previews=True)
    repository.save(selected)
    assert repository.load() == selected
    assert list((tmp_path / "settings").glob("*.tmp")) == []


def test_corrupt_settings_are_not_replaced_by_defaults(tmp_path: Path) -> None:
    """Leave user-owned bytes intact and report an actionable loading error."""
    path = tmp_path / "output.json"
    path.write_text("broken settings", encoding="utf-8")
    with pytest.raises(ValueError):
        FileOutputSavingPreferences(path).load()
    assert path.read_text(encoding="utf-8") == "broken settings"


def test_replace_failure_keeps_previous_preferences_and_removes_owned_temporary_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Model a filesystem denial at the atomic replacement boundary."""
    path = tmp_path / "output.json"
    repository = FileOutputSavingPreferences(path)
    defaults = OutputSavingPreferences()
    repository.save(defaults)
    previous = path.read_bytes()

    def reject_replace(source: Path, target: Path) -> Path:
        """Simulate an external filesystem failure without changing either destination."""
        raise PermissionError("fixture denies replacement")

    monkeypatch.setattr(Path, "replace", reject_replace)
    with pytest.raises(PermissionError):
        repository.save(replace(defaults, manage_workflow=True))
    assert path.read_bytes() == previous
    assert repository.load() == defaults
    assert list(tmp_path.glob("*.tmp")) == []
