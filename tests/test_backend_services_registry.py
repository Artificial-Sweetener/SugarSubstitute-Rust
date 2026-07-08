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
"""Active SugarCubes backend service registry tests."""

from __future__ import annotations

from sugarcubes.backend import active_backend_services, set_active_backend_services


def test_active_backend_services_returns_registered_service_graph(
    tmp_path,
    backend_services_factory,
) -> None:
    """Host adapters should retrieve the same service graph Comfy registered."""

    services = backend_services_factory(tmp_path)

    set_active_backend_services(services)

    assert active_backend_services() is services
