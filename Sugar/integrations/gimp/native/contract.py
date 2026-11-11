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
"""Validate private native messages against the same compiled schema as the companion."""

from __future__ import annotations

from dataclasses import dataclass
import json
from math import isfinite
from pathlib import Path
import re
from typing import cast


def fields(value: object) -> dict[str, object]:
    """Narrow a JSON object without allowing non-string keys across the SDK boundary."""
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise ValueError("Expected a native protocol object.")
    # JSON dictionaries carry dynamic values; every operation narrows its own fields.
    return cast(dict[str, object], value)


def text(value: object) -> str:
    """Narrow schema-validated text before a native call."""
    if not isinstance(value, str):
        raise ValueError("Expected native protocol text.")
    return value


def integer(value: object) -> int:
    """Keep JSON booleans out of native identifiers and coordinates."""
    if type(value) is not int:
        raise ValueError("Expected a native protocol integer.")
    return value


def number(value: object) -> float:
    """Narrow finite capture scales without accepting strings or booleans."""
    if type(value) not in (int, float):
        raise ValueError("Expected a native protocol number.")
    result = float(cast(int | float, value))
    if not isfinite(result):
        raise ValueError("Expected a finite native protocol number.")
    return result


def records(value: object) -> tuple[dict[str, object], ...]:
    """Narrow bounded native object lists after schema admission."""
    if not isinstance(value, list):
        raise ValueError("Expected a native protocol list.")
    return tuple(fields(item) for item in value)


def _unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Refuse ambiguous JSON instead of letting duplicate fields select an operation."""
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate native protocol field.")
        result[key] = value
    return result


def _constant(value: str) -> object:
    """Reject non-JSON floating-point constants at the decoder boundary."""
    raise ValueError("Non-finite native protocol constant.")


@dataclass(frozen=True, slots=True)
class NativeLimits:
    """Receive the shared canvas budgets compiled into this package."""

    png_bytes: int
    raster_bytes: int
    frame_bytes: int
    pending_requests: int


class NativeContract:
    """Apply the contract compiler's closed, bounded schema vocabulary to native messages."""

    def __init__(self, path: Path | None = None) -> None:
        """Load the packaged schema rather than rediscovering limits in the editor adapter."""
        self._schema = fields(
            json.loads(
                (path or Path(__file__).with_name("native.schema.json")).read_text(
                    encoding="utf-8"
                )
            )
        )
        self._definitions = fields(self._schema["$defs"])
        limits = fields(self._schema["x-limits"])
        self.limits = NativeLimits(
            integer(limits["pngBytes"]),
            integer(limits["rasterBytes"]),
            integer(limits["frameBytes"]),
            integer(limits["pendingRequests"]),
        )

    def decode(self, data: bytes) -> dict[str, object]:
        """Decode one bounded UTF-8 frame and reject ambiguous or malformed JSON."""
        if len(data) > self.limits.frame_bytes:
            raise ValueError("The native message exceeds its frame budget.")
        value: object = json.loads(
            data.decode("utf-8"), object_pairs_hook=_unique, parse_constant=_constant
        )
        self.validate(value)
        return fields(value)

    def encode(self, value: object) -> bytes:
        """Validate native replies before placing a single JSON frame on the pipe."""
        self.validate(value)
        result = (
            json.dumps(
                value, ensure_ascii=False, allow_nan=False, separators=(",", ":")
            ).encode("utf-8")
            + b"\n"
        )
        if len(result) > self.limits.frame_bytes:
            raise ValueError("The native message exceeds its frame budget.")
        return result

    def validate(self, value: object, definition: str | None = None) -> None:
        """Refuse values that disagree with the authoritative private contract."""
        schema = (
            self._schema
            if definition is None
            else fields(self._definitions[definition])
        )
        if not self._matches(schema, value, 0):
            raise ValueError("The native message does not match its contract.")

    def _matches(self, schema: dict[str, object], value: object, depth: int) -> bool:
        """Interpret only the vocabulary emitted by the shared contract compiler."""
        if depth > 32:
            return False
        if "$ref" in schema:
            reference = text(schema["$ref"])
            if not reference.startswith("#/$defs/"):
                raise ValueError("The native schema contains an external reference.")
            return self._matches(
                fields(self._definitions[reference[8:]]), value, depth + 1
            )
        if "const" in schema:
            return type(value) is type(schema["const"]) and value == schema["const"]
        choices = schema.get("enum")
        if isinstance(choices, list):
            return any(
                type(value) is type(choice) and value == choice for choice in choices
            )
        alternatives = schema.get("oneOf")
        if isinstance(alternatives, list):
            return any(
                self._matches(fields(node), value, depth + 1) for node in alternatives
            )
        kind = schema.get("type")
        if kind == "boolean":
            return type(value) is bool
        if kind in ("integer", "number"):
            if type(value) not in ((int,) if kind == "integer" else (int, float)):
                return False
            numeric = cast(int | float, value)
            return number(schema["minimum"]) <= numeric <= number(
                schema["maximum"]
            ) and isfinite(numeric)
        if kind == "string":
            if not isinstance(value, str) or sum(
                2 if ord(char) > 65535 else 1 for char in value
            ) > integer(schema["maxLength"]):
                return False
            pattern = schema.get("pattern")
            return not isinstance(pattern, str) or re.search(pattern, value) is not None
        if kind == "array":
            return (
                isinstance(value, list)
                and len(value) <= integer(schema["maxItems"])
                and all(
                    self._matches(fields(schema["items"]), item, depth + 1)
                    for item in value
                )
            )
        if kind == "object":
            if not isinstance(value, dict):
                return False
            properties = fields(schema["properties"])
            return value.keys() == properties.keys() and all(
                self._matches(fields(node), value[key], depth + 1)
                for key, node in properties.items()
            )
        raise ValueError("The native schema contains unsupported vocabulary.")
