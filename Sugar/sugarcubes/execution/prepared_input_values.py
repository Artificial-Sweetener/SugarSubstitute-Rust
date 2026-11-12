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
"""Describe immutable decoded input replacements without retaining canvas or file-system handles."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import re
from typing import Literal

from .errors import CubeLoweringError

_PREPARED_FILE = re.compile(
    r"sugarcubes/sources/source-[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\.png"
)


@dataclass(frozen=True)
class PreparedInputAsset:
    """Address one captured loader output and its already decoded, processed image asset."""

    node_path: str
    class_type: str
    field: str
    file: str
    slot: int
    kind: Literal["image", "mask"]
    prepared_file: str

    def __post_init__(self) -> None:
        """Keep asset identities bounded and restrict prepared files to owned immutable uploads."""
        for name, limit in (
            ("node_path", 4096),
            ("class_type", 512),
            ("field", 128),
            ("file", 4096),
            ("prepared_file", 4096),
        ):
            value: object = getattr(self, name)
            if (
                not isinstance(value, str)
                or not value
                or len(value) > limit
                or any(
                    ord(character) < 32 or ord(character) == 127 for character in value
                )
            ):
                raise _invalid("Invalid prepared input identity.")
        if (
            type(self.slot) is not int
            or not 0 <= self.slot <= 1024
            or self.kind not in ("image", "mask")
        ):
            raise _invalid("Invalid prepared input socket.")
        if not _PREPARED_FILE.fullmatch(self.prepared_file):
            raise _invalid(
                "Prepared inputs must use an immutable SugarCubes source upload."
            )


def parse_prepared_input_assets(value: object) -> tuple[PreparedInputAsset, ...]:
    """Narrow HTTP assets into bounded values before execution can inspect their source identities."""
    if value is None:
        return ()
    if not isinstance(value, list) or len(value) > 128:
        raise _invalid("Invalid prepared input collection.")
    result: list[PreparedInputAsset] = []
    used: set[tuple[str, int]] = set()
    for entry in value:
        if not isinstance(entry, Mapping):
            raise _invalid("Invalid prepared input asset.")
        record: Mapping[object, object] = entry
        slot = record.get("slot")
        kind = record.get("kind")
        if type(slot) is not int or kind not in ("image", "mask"):
            raise _invalid("Invalid prepared input socket.")
        asset = PreparedInputAsset(
            node_path=_text(record, "nodePath"),
            class_type=_text(record, "classType"),
            field=_text(record, "field"),
            file=_text(record, "file"),
            slot=slot,
            kind="image" if kind == "image" else "mask",
            prepared_file=_text(record, "preparedFile"),
        )
        key = (asset.node_path, asset.slot)
        if key in used:
            raise _invalid("One source socket cannot have competing prepared inputs.")
        used.add(key)
        result.append(asset)
    return tuple(result)


def _text(record: Mapping[object, object], name: str) -> str:
    """Reject dynamic values before they enter a typed input replacement."""
    value = record.get(name)
    if not isinstance(value, str):
        raise _invalid("Invalid prepared input identity.")
    return value


def _invalid(message: str) -> CubeLoweringError:
    """Keep all input preparation failures on the existing execution diagnostic path."""
    return CubeLoweringError("execution.prepared_inputs.invalid", message)
