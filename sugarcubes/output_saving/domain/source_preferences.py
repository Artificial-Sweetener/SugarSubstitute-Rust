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
"""Validate workflow-local ordinary-source choices without retaining host graph objects."""

from collections.abc import Mapping
from dataclasses import dataclass
from types import MappingProxyType


@dataclass(frozen=True, slots=True)
class SourceSavingPreference:
    """Keep each execution path independent from shared subgraph definitions and cube order."""

    enabled: bool = True
    label: str | None = None


def decode_source_preferences(value: object) -> Mapping[str, SourceSavingPreference]:
    """Reject malformed explicit policy rather than silently restoring native saving."""
    if value is None:
        return MappingProxyType({})
    if (
        not isinstance(value, Mapping)
        or type(value.get("schema_version")) is not int
        or value.get("schema_version") != 1
    ):
        raise ValueError("Unsupported workflow output-source preferences version.")
    sources = value.get("sources")
    if not isinstance(sources, Mapping) or len(sources) > 10000:
        raise ValueError(
            "Workflow output-source preferences must contain a bounded source object."
        )
    result: dict[str, SourceSavingPreference] = {}
    for path, entry in sources.items():
        if not isinstance(path, str) or not path or len(path) > 4096 or "\x00" in path:
            raise ValueError(
                "An output source preference has an invalid execution identity."
            )
        if not isinstance(entry, Mapping):
            raise ValueError("Each output source preference must be an object.")
        enabled, label = entry.get("enabled", True), entry.get("label")
        if type(enabled) is not bool:
            raise ValueError("Output source saving must be a boolean.")
        if label is not None and (
            not isinstance(label, str) or len(label) > 4096 or "\x00" in label
        ):
            raise ValueError(
                "Output source labels must be bounded text without null characters."
            )
        result[path] = SourceSavingPreference(
            enabled, label.strip() or None if isinstance(label, str) else None
        )
    return MappingProxyType(result)
