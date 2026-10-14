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
"""Safeguard Substitute saving preferences and independent graph opt-in."""

from copy import deepcopy

import pytest

from sugarcubes.output_saving.domain.preferences import OutputSavingPreferences
from sugarcubes.output_saving.domain.preference_codec import (
    decode_preferences,
    encode_preferences,
    import_substitute_preferences,
)


@pytest.mark.parametrize("version", ["1", "2", "3", "4"])
def test_import_preserves_saving_choices_without_enabling_graph_management(
    version: str,
) -> None:
    """Retain exact paths and sizing while leaving the original settings untouched."""
    payload = {
        "schema_version": version,
        "organization": {
            "output_root": "E:\\Images\\Generated",
            "path_pattern": "{date}\\{run}_{cube#}_{workflow}_{source}_{index}",
        },
        "jpeg": {
            "enabled": True,
            "sizing_mode": "target_size",
            "quality": 87,
            "target_size_kib": 2048,
        },
        "transfer": {"preferred_format": "companion_jpeg"},
        "persistence_mode": "final_cube",
    }
    original = deepcopy(payload)
    imported = import_substitute_preferences(payload)
    assert imported.organization.output_root == "E:\\Images\\Generated"
    assert (
        imported.organization.path_pattern
        == "{date}\\{run}_{cube#}_{workflow}_{source}_{index}"
    )
    assert imported.jpeg.enabled
    assert imported.jpeg.quality == 87
    assert imported.jpeg.target_size_kib == 2048
    assert imported.jpeg.sizing_mode == "target_size"
    assert imported.transfer_format == "companion_jpeg"
    assert imported.persistence_mode == "final_cube"
    assert not imported.manage_workflow
    assert not imported.save_previews
    assert decode_preferences(encode_preferences(imported)) == imported
    assert payload == original


def test_legacy_organization_import_and_defaults() -> None:
    """Keep early folder-only settings compatible with current saving defaults."""
    imported = import_substitute_preferences(
        {"output_root": None, "path_pattern": "{workflow}/{source}"}
    )
    defaults = OutputSavingPreferences()
    assert imported.organization.output_root is None
    assert imported.organization.path_pattern == "{workflow}/{source}"
    assert imported.jpeg == defaults.jpeg
    assert imported.persistence_mode == "all"
    assert not imported.manage_workflow
    assert defaults.jpeg.quality == 100
    assert defaults.jpeg.target_size_kib == 1024
    assert defaults.transfer_format == "canonical_png"


@pytest.mark.parametrize(
    "payload",
    [
        None,
        [],
        {},
        {"schema_version": "4"},
        {"schema_version": "4", "theme": "dark"},
        {"schema_version": "4", "organization": {"download_root": "E:\\Models"}},
        {"schema_version": "4", "jpeg": {}, "transfer": {}},
        {"schema_version": "5"},
        {"jpeg": {"enabled": "false"}},
        {"jpeg": {"quality": True}},
        {"jpeg": {"quality": 101}},
        {"jpeg": {"target_size_kib": 0}},
        {"transfer": {"preferred_format": "webp"}},
        {"persistence_mode": "last_unmuted"},
        {"organization": {"path_pattern": "../escape"}},
    ],
)
def test_import_rejects_unsupported_or_invalid_settings(payload: object) -> None:
    """Fail visibly rather than silently rewriting user choices during migration."""
    with pytest.raises(ValueError):
        import_substitute_preferences(payload)


def test_native_preferences_require_their_versioned_format() -> None:
    """Keep native persistence and foreign migration as explicit separate boundaries."""
    with pytest.raises(ValueError):
        decode_preferences({"schema_version": "4"})
