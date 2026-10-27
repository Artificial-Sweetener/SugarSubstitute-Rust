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
"""Qualify live native document scopes without reading or modifying another process's memory."""

from __future__ import annotations

import os
import sys

import pytest

from integrations.gimp.native.identity import windows_process_identity

pytestmark = pytest.mark.skipif(
    sys.platform != "win32", reason="The native companion targets Windows."
)


def test_native_process_scope_is_stable_for_the_same_live_process() -> None:
    """Read public Windows process metadata while preserving the same host identity across viewers."""
    identity = windows_process_identity(os.getpid())
    assert identity.startswith(f"{os.getpid():x}-")
    assert identity == windows_process_identity(os.getpid())


@pytest.mark.parametrize("process_id", [0, -1, True])
def test_invalid_native_process_identifiers_are_refused(process_id: int) -> None:
    """Refuse malformed process identifiers before opening a query handle."""
    with pytest.raises(ValueError, match="positive"):
        windows_process_identity(process_id)
