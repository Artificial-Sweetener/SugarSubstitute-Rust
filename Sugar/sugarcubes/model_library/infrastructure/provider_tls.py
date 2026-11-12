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
"""Share verified provider TLS trust independently of machine-specific certificate stores."""

from __future__ import annotations

from functools import lru_cache
import ssl

import certifi


@lru_cache(maxsize=1)
def provider_tls_context() -> ssl.SSLContext:
    """Verify provider identity against the Mozilla trust bundle used by Substitute's transport."""
    return ssl.create_default_context(cafile=certifi.where())
