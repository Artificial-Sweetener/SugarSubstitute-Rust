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
"""Prove settings migration validates server paths before replacing preferences."""

from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.output_saving.application.settings_service import (
    OutputSavingSettingsService,
)
from sugarcubes.output_saving.domain.preferences import (
    OutputOrganization,
    OutputSavingPreferences,
)
from sugarcubes.output_saving.infrastructure.preference_repository import (
    FileOutputSavingPreferences,
)
from sugarcubes.output_saving.infrastructure.settings_paths import (
    HostSavingSettingsPaths,
)


def test_migration_preserves_an_existing_explicit_management_choice(
    tmp_path: Path,
) -> None:
    """Import foreign cube policy without either enabling or disabling existing graph management."""
    repository = FileOutputSavingPreferences(tmp_path / "settings.json")
    service = OutputSavingSettingsService(
        repository, HostSavingSettingsPaths(tmp_path / "outputs")
    )
    previous = replace(
        OutputSavingPreferences(), manage_workflow=True, save_previews=True
    )
    service.save(previous)
    imported = service.import_substitute(
        {"schema_version": "4", "persistence_mode": "final_cube"}
    )
    assert imported.manage_workflow
    assert imported.save_previews
    assert imported.persistence_mode == "final_cube"
    assert repository.load() == imported


def test_invalid_migrated_destination_leaves_original_settings_intact(
    tmp_path: Path,
) -> None:
    """Fail server-side path validation before overwriting any user preferences."""
    repository = FileOutputSavingPreferences(tmp_path / "settings.json")
    service = OutputSavingSettingsService(repository, HostSavingSettingsPaths(tmp_path))
    service.save(OutputSavingPreferences())
    previous = (tmp_path / "settings.json").read_bytes()
    with pytest.raises(ValueError):
        service.import_substitute({"output_root": "relative/folder"})
    assert (tmp_path / "settings.json").read_bytes() == previous


def test_example_and_proposed_validation_do_not_create_output_folders(
    tmp_path: Path,
) -> None:
    """Render settings examples without reserving run numbers or writing image files."""
    output = tmp_path / "not-created"
    service = OutputSavingSettingsService(
        FileOutputSavingPreferences(
            tmp_path / "not-created-settings" / "settings.json"
        ),
        HostSavingSettingsPaths(output),
    )
    assert service.preview() == str(
        output / "2026-05-01" / "007_01_my_workflow_main_output.png"
    )
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []


def test_existing_file_cannot_be_configured_as_output_root(tmp_path: Path) -> None:
    """Report a destination error before saving unusable policy."""
    file = tmp_path / "ordinary-file"
    file.write_text("user content", encoding="utf-8")
    preferences = OutputSavingPreferences(
        organization=OutputOrganization(output_root=str(file))
    )
    with pytest.raises(ValueError):
        HostSavingSettingsPaths(tmp_path).validate(preferences)
    assert file.read_text(encoding="utf-8") == "user content"
