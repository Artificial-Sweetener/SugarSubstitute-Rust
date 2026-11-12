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
"""Reject remote and cross-origin desktop requests independently of advertised host capabilities."""

from unittest.mock import Mock

import pytest
from aiohttp.test_utils import make_mocked_request

from sugarcubes.output_saving.infrastructure.local_file_access import local_file_request


@pytest.mark.parametrize(
    "peer,host,origin,allowed",
    [
        ("127.0.0.1", "127.0.0.1:8189", "http://127.0.0.1:8189", True),
        ("::1", "[::1]:8189", "http://[::1]:8189", True),
        ("127.0.0.1", "localhost:8189", "http://localhost:8189", True),
        ("203.0.113.1", "localhost:8189", "http://localhost:8189", False),
        (
            "127.0.0.1",
            "localhost.attacker.example",
            "http://localhost.attacker.example",
            False,
        ),
        ("127.0.0.1", "127.0.0.1:8189", "http://127.0.0.1:8190", False),
        ("127.0.0.1", "localhost:8189", "https://attacker.example", False),
        ("127.0.0.1", "localhost:8189", "null", False),
    ],
)
def test_desktop_request_requires_matching_loopback_origin(
    peer: str, host: str, origin: str, allowed: bool
) -> None:
    """Use the real request adapter with only its transport peer supplied externally."""
    transport = Mock()
    transport.get_extra_info.side_effect = lambda name: (
        (peer, 1234) if name == "peername" else None
    )
    request = make_mocked_request(
        "POST", "/reveal", headers={"Host": host, "Origin": origin}, transport=transport
    )
    assert local_file_request(request, require_origin=True) is allowed


@pytest.mark.parametrize(
    "header", ["Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Real-IP"]
)
def test_forwarded_connections_never_enable_desktop_operations(header: str) -> None:
    """Refuse proxy hints even when a reverse proxy connects through loopback."""
    transport = Mock()
    transport.get_extra_info.side_effect = lambda name: (
        ("127.0.0.1", 1234) if name == "peername" else None
    )
    request = make_mocked_request(
        "POST",
        "/reveal",
        headers={
            "Host": "localhost:8189",
            "Origin": "http://localhost:8189",
            header: "remote",
        },
        transport=transport,
    )
    assert not local_file_request(request, require_origin=True)
