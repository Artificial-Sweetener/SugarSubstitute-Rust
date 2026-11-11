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
"""Prove persisted source policy validation is independent from graph and renderer lifecycles."""

import pytest

from sugarcubes.output_saving.domain.source_preferences import decode_source_preferences


def test_source_preferences_are_immutable_literal_and_instance_scoped() -> None:
    """Distinguish two instances of one nested saver without modifying their shared definition."""
    preferences = decode_source_preferences(
        {
            "schema_version": 1,
            "sources": {
                "a:save": {"enabled": False, "label": " <one> "},
                "b:save": {"enabled": True, "label": None},
            },
        }
    )
    assert not preferences["a:save"].enabled
    assert preferences["a:save"].label == "<one>"
    assert preferences["b:save"].enabled
    assert preferences["b:save"].label is None
    assert decode_source_preferences(None) == {}


@pytest.mark.parametrize(
    "value",
    [
        [],
        {},
        {"schema_version": True, "sources": {}},
        {"schema_version": 2, "sources": {}},
        {"schema_version": 1, "sources": []},
        {"schema_version": 1, "sources": {"": {}}},
        {"schema_version": 1, "sources": {"save": {"enabled": "yes"}}},
        {"schema_version": 1, "sources": {"save": {"label": []}}},
        {"schema_version": 1, "sources": {"save": {"label": "\x00"}}},
    ],
)
def test_explicit_invalid_source_policy_is_rejected(value: object) -> None:
    """Prevent corrupt source choices from silently enabling writes or changing source identity."""
    with pytest.raises(ValueError):
        decode_source_preferences(value)
