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
"""Model immutable saving settings with independent cube and ordinary-node scopes."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum

from .path_pattern import DEFAULT_PATH_PATTERN, validate_pattern


class PersistenceMode(StrEnum):
    """Select every active cube or the final active cube before applying local mutes."""

    ALL = "all"
    FINAL_CUBE = "final_cube"


class JpegSizingMode(StrEnum):
    """Choose Substitute's fixed-quality or approximate-size companion encoding."""

    QUALITY = "quality"
    TARGET_SIZE = "target_size"


class TransferFormat(StrEnum):
    """Select the outbound representation without changing the canonical PNG."""

    CANONICAL_PNG = "canonical_png"
    COMPANION_JPEG = "companion_jpeg"


@dataclass(frozen=True, slots=True)
class OutputOrganization:
    """Retain the exact user root while delegating host path validation to infrastructure."""

    output_root: str | None = None
    path_pattern: str = DEFAULT_PATH_PATTERN

    def __post_init__(self) -> None:
        """Reject malformed patterns and unusable root text at the preference boundary."""
        validate_pattern(self.path_pattern)
        if self.output_root is not None and (
            not self.output_root.strip() or "\x00" in self.output_root
        ):
            raise ValueError(
                "Output folder must be a nonempty path without null characters."
            )


@dataclass(frozen=True, slots=True)
class JpegPreferences:
    """Preserve companion sizing values when switching modes or disabling companions."""

    enabled: bool = False
    sizing_mode: JpegSizingMode = JpegSizingMode.QUALITY
    quality: int = 100
    target_size_kib: int = 1024

    def __post_init__(self) -> None:
        """Reject invalid values rather than silently changing imported settings."""
        if type(self.enabled) is not bool:
            raise ValueError("JPEG enabled must be a boolean.")
        if type(self.quality) is not int or not 1 <= self.quality <= 100:
            raise ValueError("JPEG quality must be an integer between 1 and 100.")
        if type(self.target_size_kib) is not int or self.target_size_kib < 1:
            raise ValueError("JPEG target size must be a positive integer in KiB.")


@dataclass(frozen=True, slots=True)
class OutputSavingPreferences:
    """Keep cube saving always available while requiring opt-in for ordinary output management."""

    organization: OutputOrganization = field(default_factory=OutputOrganization)
    jpeg: JpegPreferences = field(default_factory=JpegPreferences)
    persistence_mode: PersistenceMode = PersistenceMode.ALL
    transfer_format: TransferFormat = TransferFormat.CANONICAL_PNG
    manage_workflow: bool = False
    save_previews: bool = False

    @property
    def effective_transfer_format(self) -> TransferFormat:
        """Export PNG when companions are disabled, retaining the user's stored preference."""
        return (
            self.transfer_format if self.jpeg.enabled else TransferFormat.CANONICAL_PNG
        )
