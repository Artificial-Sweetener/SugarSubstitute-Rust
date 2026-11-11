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
"""Validate the installed SDK's opaque display association before passing it to another process."""

from __future__ import annotations

from types import SimpleNamespace
from unittest.mock import patch

import pytest

from integrations.gimp.native.sdk import host_window_handle


@pytest.mark.parametrize("handle", [1, 38282442, 2**32 + 7])
def test_native_handle_preserves_the_full_sdk_value(handle: int) -> None:
    """Decode the public little-endian pointer without truncation or window enumeration."""
    value = SimpleNamespace(get_data=lambda: handle.to_bytes(8, "little"))
    with patch(
        "integrations.gimp.native.sdk.import_module",
        return_value=SimpleNamespace(progress_get_window_handle=lambda: value),
    ):
        assert host_window_handle() == handle


def test_headless_sdk_has_no_window_to_associate() -> None:
    """Keep native capture tests available when GIMP deliberately has no GUI display."""
    with patch(
        "integrations.gimp.native.sdk.import_module",
        return_value=SimpleNamespace(progress_get_window_handle=lambda: None),
    ):
        assert host_window_handle() is None


def test_headless_nullable_handle_can_be_boxed_as_empty_gbytes() -> None:
    """Accept the exact empty GBytes representation observed in installed GIMP 3.2.6."""
    value = SimpleNamespace(get_data=lambda: b"", get_size=lambda: 0)
    with patch(
        "integrations.gimp.native.sdk.import_module",
        return_value=SimpleNamespace(progress_get_window_handle=lambda: value),
    ):
        assert host_window_handle() is None


@pytest.mark.parametrize("data", [b"", bytes(4), bytes(8), "not bytes", 42])
def test_malformed_sdk_handle_is_refused(data: object) -> None:
    """Reject invalid SDK values before any native viewer process or window exists."""
    value = SimpleNamespace(get_data=lambda: data)
    with patch(
        "integrations.gimp.native.sdk.import_module",
        return_value=SimpleNamespace(progress_get_window_handle=lambda: value),
    ):
        with pytest.raises(RuntimeError, match="window handle"):
            host_window_handle()


def test_missing_sdk_association_is_actionable() -> None:
    """Identify unsupported installed bindings rather than choosing another application window."""
    with patch(
        "integrations.gimp.native.sdk.import_module", return_value=SimpleNamespace()
    ):
        with pytest.raises(RuntimeError, match="does not expose"):
            host_window_handle()
