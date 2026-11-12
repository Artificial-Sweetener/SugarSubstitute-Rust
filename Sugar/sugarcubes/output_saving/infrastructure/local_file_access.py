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
"""Qualify loopback same-origin browser requests before enabling desktop file operations."""

from ipaddress import ip_address
from urllib.parse import urlsplit

from aiohttp import web


def local_file_request(request: web.Request, *, require_origin: bool) -> bool:
    """Refuse remote peers, proxy headers, foreign origins and non-loopback hostnames."""
    if any(
        name in request.headers
        for name in ("Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Real-IP")
    ):
        return False
    peer = request.remote
    if not peer or not _loopback(peer):
        return False
    try:
        host = urlsplit(f"{request.scheme}://{request.host}")
        if not host.hostname or not _loopback(host.hostname):
            return False
        origin_text = request.headers.get("Origin")
        if not origin_text:
            return (
                not require_origin
                and request.headers.get("Sec-Fetch-Site", "same-origin")
                == "same-origin"
            )
        origin = urlsplit(origin_text)
        return (
            origin.scheme == host.scheme
            and origin.netloc.casefold() == host.netloc.casefold()
            and origin.path == ""
            and not origin.query
            and not origin.fragment
            and request.headers.get("Sec-Fetch-Site", "same-origin") == "same-origin"
        )
    except ValueError:
        return False


def _loopback(host: str) -> bool:
    """Accept exact localhost and actual loopback addresses rather than substring matches."""
    if host.casefold() == "localhost":
        return True
    try:
        return ip_address(host).is_loopback
    except ValueError:
        return False
