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
"""Record the pre-coordinator SugarCubes host boundary."""

from __future__ import annotations

from sugarcubes import host_api


def test_host_api_v1_has_no_competing_execution_owner() -> None:
    """Prove the additive v2 execution contract starts from an unclaimed boundary."""

    assert host_api.HOST_API_VERSION == 1
    assert not hasattr(host_api, "execution")
