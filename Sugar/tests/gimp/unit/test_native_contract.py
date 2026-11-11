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
"""Reject malformed native frames before values reach GIMP SDK calls."""

from __future__ import annotations

from math import inf, nan

import pytest

from integrations.gimp.native.contract import NativeContract


def test_native_tattoos_use_unsigned_identity_without_widening_request_ids() -> None:
    """Preserve GIMP's full tattoo range independently of signed process-local request tickets."""
    contract = NativeContract()
    contract.validate({"tattoo": 4294967295, "visible": True}, "NativeVisibility")
    for tattoo in (0, -1, 4294967296, True):
        with pytest.raises(ValueError):
            contract.validate({"tattoo": tattoo, "visible": True}, "NativeVisibility")
    with pytest.raises(ValueError):
        contract.decode(
            contract.encode(
                {"type": "request", "id": 2147483648, "command": {"type": "context"}}
            )
        )


@pytest.mark.parametrize("scale", [1, 0.5, 0.0001])
def test_native_capture_scale_remains_a_number(scale: float) -> None:
    """Preserve fractional downsampling through JSON and the shared compiler vocabulary."""
    contract = NativeContract()
    command = {
        "type": "snapshot.selection",
        "token": "fixture",
        "width": 4,
        "height": 3,
        "scale": scale,
    }
    frame = {"type": "request", "id": 7, "command": command}
    assert contract.decode(contract.encode(frame)) == frame


@pytest.mark.parametrize("scale", [True, "0.5", inf, -inf, nan, -0.1, 1.001])
def test_native_capture_rejects_invalid_numeric_values(scale: object) -> None:
    """Refuse coercion and non-finite values before a native allocation."""
    with pytest.raises(ValueError, match="contract"):
        NativeContract().validate(
            {
                "type": "snapshot.selection",
                "token": "fixture",
                "width": 4,
                "height": 3,
                "scale": scale,
            },
            "NativeCommand",
        )


@pytest.mark.parametrize(
    "frame",
    [
        b'{"type":"request","id":1,"id":2,"command":{"type":"context"}}',
        b'{"type":"request","id":1,"command":{"type":"context","extra":true}}',
        b'{"type":"request","id":true,"command":{"type":"context"}}',
        b'{"type":"request","id":1,"command":{"type":"context"},"extra":true}',
        b'{"type":"request","id":1,"command":{"type":"snapshot.selection","token":"bad\\n","width":1,"height":1,"scale":0.5}}',
        b'{"type":"request","id":1,"command":{"type":"snapshot.selection","token":"ok","width":1,"height":1,"scale":NaN}}',
        b"\xff",
    ],
)
def test_invalid_private_frames_fail_closed(frame: bytes) -> None:
    """Refuse ambiguous fields, invalid envelopes, unknown fields and invalid UTF-8."""
    with pytest.raises((ValueError, UnicodeError)):
        NativeContract().decode(frame)


def test_native_contract_counts_text_in_companion_utf16_units() -> None:
    """Keep generated TypeScript and Python string bounds aligned for astral characters."""
    contract = NativeContract()
    valid = {"kind": "failure", "status": "error", "message": "🧊" * 256}
    contract.validate(valid, "NativeResult")
    with pytest.raises(ValueError, match="contract"):
        contract.validate({**valid, "message": "🧊" * 257}, "NativeResult")


def test_native_frames_are_bounded_before_json_decoding() -> None:
    """Reject a too-large frame without allocating its parsed graph."""
    contract = NativeContract()
    with pytest.raises(ValueError, match="frame budget"):
        contract.decode(b" " * (contract.limits.frame_bytes + 1))
