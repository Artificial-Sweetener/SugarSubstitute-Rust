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
"""Validate CivitAI secret material before storage or authenticated transport."""

from __future__ import annotations


def validated_civitai_key(key: str) -> str:
    """Normalize explicit keys while preventing header injection and unbounded persistence."""
    if len(key) > 4096 or "\r" in key or "\n" in key or "\x00" in key:
        raise ValueError("The CivitAI credential is invalid.")
    normalized = key.strip()
    if not normalized:
        raise ValueError("CivitAI API key cannot be empty.")
    return normalized
