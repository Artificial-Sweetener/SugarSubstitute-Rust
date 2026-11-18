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
"""Encode values and names exactly as SugarSubstitute persisted them in Sugar scripts.

A project written here must open in SugarSubstitute, and one written there must come back
unchanged, so these rules are that application's own: Python spellings for scalars, raw
triple-quoted text for multi-line strings, and a bare name wherever a name is alphanumeric.
"""

from __future__ import annotations

import json
import re

_VERSION_PIN = re.compile(r"^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*$")


def encode_literal(value: object) -> str:
    """Return the persisted spelling of one field value.

    Booleans and numbers use Python's spelling (`True`, `7.0`, `1e-05`), a list is compact JSON,
    and every other value that is not a string is spelled as Python prints it.
    """

    if value is None:
        return "null"
    if isinstance(value, list):
        return json.dumps(value) if value else "[]"
    if isinstance(value, str):
        return _encode_string(value)
    return str(value)


def encode_segment(segment: str) -> str:
    """Return one path segment bare when it is alphanumeric apart from underscores, else quoted."""

    if segment.replace("_", "").isalnum():
        return segment
    return f'"{_escape(segment)}"'


def encode_version_pin(version: str) -> str:
    """Return a version pin bare when it is dotted alphanumerics, else as a string literal."""

    return version if _VERSION_PIN.fullmatch(version) else encode_literal(version)


def _encode_string(value: str) -> str:
    """Write multi-line text raw between triple quotes whenever that reads back unchanged."""

    if (
        "\n" in value
        and "\r" not in value
        and '"""' not in value
        and not value.endswith('"')
    ):
        return f'"""{value}"""'
    return f'"{_escape(value)}"'


def _escape(value: str) -> str:
    """Escape the five characters a single-line quoted string cannot hold as themselves."""

    return (
        value.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\r", "\\r")
        .replace("\n", "\\n")
        .replace("\t", "\\t")
    )


__all__ = ["encode_literal", "encode_segment", "encode_version_pin"]
