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
"""Atomically persist validated output settings without modifying migration sources."""

from __future__ import annotations

import json
import logging
from pathlib import Path
import tempfile

from ..domain.preference_codec import decode_preferences, encode_preferences
from ..domain.preferences import OutputSavingPreferences

_LOGGER = logging.getLogger(__name__)


class FileOutputSavingPreferences:
    """Own one server-side versioned preference file with fail-closed invalid-data loading."""

    def __init__(self, path: Path) -> None:
        """Receive a composition-owned settings destination rather than discovering global paths."""
        self._path = path

    def load(self) -> OutputSavingPreferences:
        """Return defaults only for an absent file, preserving invalid existing bytes for recovery."""
        if not self._path.exists():
            return OutputSavingPreferences()
        try:
            payload: object = json.loads(self._path.read_text(encoding="utf-8"))
            return decode_preferences(payload)
        except (OSError, ValueError):
            _LOGGER.exception(
                "Output saving preferences could not be loaded; existing settings retained."
            )
            raise

    def save(self, preferences: OutputSavingPreferences) -> None:
        """Validate and replace one file atomically, leaving the previous file intact on failure."""
        payload = encode_preferences(preferences)
        decode_preferences(payload)
        self._path.parent.mkdir(parents=True, exist_ok=True)
        temporary: Path | None = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=self._path.parent,
                prefix=".output-settings-",
                suffix=".tmp",
                delete=False,
            ) as stream:
                temporary = Path(stream.name)
                json.dump(payload, stream, indent=2, ensure_ascii=False)
                stream.write("\n")
                stream.flush()
            temporary.replace(self._path)
        except (OSError, ValueError):
            _LOGGER.exception(
                "Output saving preferences could not be saved; previous settings retained."
            )
            raise
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
