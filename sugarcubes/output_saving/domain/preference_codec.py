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
"""Validate native saving persistence and explicit Substitute settings migration."""

from __future__ import annotations

from collections.abc import Mapping
from typing import TypeVar
from enum import StrEnum

from .preferences import (
    JpegPreferences,
    JpegSizingMode,
    OutputOrganization,
    OutputSavingPreferences,
    PersistenceMode,
    TransferFormat,
)

FORMAT = "sugarcubes.output_saving"
SCHEMA_VERSION = 1
_Enum = TypeVar("_Enum", bound=StrEnum)


def decode_preferences(value: object) -> OutputSavingPreferences:
    """Require the native format/version before accepting persisted saving settings."""
    payload = _record(value, "Output saving settings")
    if (
        payload.get("format") != FORMAT
        or type(payload.get("schema_version")) is not int
        or payload.get("schema_version") != SCHEMA_VERSION
    ):
        raise ValueError(
            "Unsupported SugarCubes output saving settings format or version."
        )
    return _decode_fields(payload, include_management=True)


def import_substitute_preferences(value: object) -> OutputSavingPreferences:
    """Import supported Substitute settings without opting into ordinary-node management."""
    payload = _record(value, "Substitute output settings")
    version = payload.get("schema_version")
    if version is not None and (
        not isinstance(version, str) or version not in {"1", "2", "3", "4"}
    ):
        raise ValueError("Unsupported Substitute output settings version.")
    return _decode_fields(payload, include_management=False)


def encode_preferences(preferences: OutputSavingPreferences) -> dict[str, object]:
    """Write one versioned aggregate while retaining inactive sizing and transfer choices."""
    return {
        "format": FORMAT,
        "schema_version": SCHEMA_VERSION,
        "organization": {
            "output_root": preferences.organization.output_root,
            "path_pattern": preferences.organization.path_pattern,
        },
        "jpeg": {
            "enabled": preferences.jpeg.enabled,
            "sizing_mode": preferences.jpeg.sizing_mode.value,
            "quality": preferences.jpeg.quality,
            "target_size_kib": preferences.jpeg.target_size_kib,
        },
        "transfer": {"preferred_format": preferences.transfer_format.value},
        "persistence_mode": preferences.persistence_mode.value,
        "manage_workflow": preferences.manage_workflow,
        "save_previews": preferences.save_previews,
    }


def _decode_fields(
    payload: Mapping[str, object], *, include_management: bool
) -> OutputSavingPreferences:
    """Narrow external values before constructing immutable domain preferences."""
    defaults = OutputSavingPreferences()
    organization = _record(payload.get("organization", payload), "Output organization")
    jpeg = _record(payload.get("jpeg", {}), "JPEG companions")
    transfer = _record(payload.get("transfer", {}), "Output transfer")
    root = organization.get("output_root")
    if root is not None and not isinstance(root, str):
        raise ValueError("Output folder must be a path string or null.")
    return OutputSavingPreferences(
        organization=OutputOrganization(
            output_root=root,
            path_pattern=_text(
                organization.get("path_pattern", defaults.organization.path_pattern),
                "Output pattern",
            ),
        ),
        jpeg=JpegPreferences(
            enabled=_boolean(jpeg.get("enabled", False), "JPEG enabled"),
            sizing_mode=_enum(
                JpegSizingMode, jpeg.get("sizing_mode", defaults.jpeg.sizing_mode.value)
            ),
            quality=_integer(
                jpeg.get("quality", defaults.jpeg.quality), "JPEG quality"
            ),
            target_size_kib=_integer(
                jpeg.get("target_size_kib", defaults.jpeg.target_size_kib),
                "JPEG target size",
            ),
        ),
        persistence_mode=_enum(
            PersistenceMode,
            payload.get("persistence_mode", defaults.persistence_mode.value),
        ),
        transfer_format=_enum(
            TransferFormat,
            transfer.get("preferred_format", defaults.transfer_format.value),
        ),
        manage_workflow=(
            _boolean(payload.get("manage_workflow", False), "Workflow management")
            if include_management
            else False
        ),
        save_previews=(
            _boolean(payload.get("save_previews", False), "Preview saving")
            if include_management
            else False
        ),
    )


def _record(value: object, name: str) -> Mapping[str, object]:
    """Accept only named object fields at the serialization boundary."""
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise ValueError(f"{name} must be an object with string keys.")
    return {key: item for key, item in value.items() if isinstance(key, str)}


def _boolean(value: object, name: str) -> bool:
    """Reject truthy substitutes for actual persisted booleans."""
    if not isinstance(value, bool):
        raise ValueError(f"{name} must be a boolean.")
    return value


def _integer(value: object, name: str) -> int:
    """Reject fractional values and Python's boolean-as-integer ambiguity."""
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{name} must be an integer.")
    return value


def _text(value: object, name: str) -> str:
    """Reject malformed text without changing meaningful user whitespace or casing."""
    if not isinstance(value, str):
        raise ValueError(f"{name} must be a string.")
    return value


def _enum(enum_type: type[_Enum], value: object) -> _Enum:
    """Require supported external enum values rather than falling back silently."""
    return enum_type(_text(value, enum_type.__name__))
