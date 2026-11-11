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
"""Own provider-specific download permission independently of catalog links and HTTP."""

from __future__ import annotations

import ipaddress
import re
from urllib.parse import SplitResult, urlsplit

_OPENMODELDB_DIRECT_HOSTS = frozenset(
    {
        "github.com",
        "huggingface.co",
        "objectstorage.us-phoenix-1.oraclecloud.com",
        "raw.githubusercontent.com",
    }
)


def approved_civitai_download(url: str) -> bool:
    """Admit canonical provider routes without caller-supplied tokens or credentials."""
    value = _https_address(url)
    return (
        value is not None
        and value.hostname == "civitai.com"
        and bool(re.fullmatch(r"/api/download/models/[1-9][0-9]*", value.path))
        and "token" not in value.query.lower()
    )


def approved_openmodeldb_download(url: str) -> bool:
    """Match Substitute's reviewed direct-download hosts without granting metadata links IO."""
    value = _https_address(url)
    return value is not None and value.hostname in _OPENMODELDB_DIRECT_HOSTS


def approved_artifact_start(url: str, provider: str) -> bool:
    """Require the offer's authoritative provider to approve the initial transfer route."""
    if provider == "civitai":
        return approved_civitai_download(url)
    if provider == "openmodeldb":
        return approved_openmodeldb_download(url)
    return False


def approved_artifact_redirect(url: str, provider: str) -> bool:
    """Keep provider redirect semantics separate while transport enforces public DNS."""
    value = _https_address(url)
    if value is None:
        return False
    host = value.hostname or ""
    if provider == "openmodeldb":
        return True
    return provider == "civitai" and (
        host in ("civitai.com", "model-delivery.civitai.com", "b2.civitai.com")
        or (
            host.startswith("civitai-delivery-worker-prod.")
            and host.endswith(".r2.cloudflarestorage.com")
        )
    )


def _https_address(url: str) -> SplitResult | None:
    """Reject malformed, local, credential-bearing, or downgraded transport addresses."""
    try:
        value = urlsplit(url)
        host = value.hostname
        if (
            value.scheme != "https"
            or not host
            or value.port not in (None, 443)
            or value.username
            or value.password
            or value.fragment
            or host == "localhost"
            or host.endswith((".localhost", ".local"))
        ):
            return None
        try:
            address = ipaddress.ip_address(host)
        except ValueError:
            return value
        return value if address.is_global else None
    except ValueError:
        return None
