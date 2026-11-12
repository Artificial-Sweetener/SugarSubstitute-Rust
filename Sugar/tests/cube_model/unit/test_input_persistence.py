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
"""Prove authored-default persistence excludes machine-specific selections."""

from sugarcubes.cube_model.input_persistence import should_store_authored_value


def test_generic_device_selection_is_not_a_portable_authored_default() -> None:
    """Exclude hardware selection independently of the owning node class."""

    assert not should_store_authored_value(
        "AnyDeviceAwareNode",
        "device",
        field_spec=[["cpu", "cuda:0"], {"default": "cpu"}],
    )
