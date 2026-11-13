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
"""Normalize primitive values consumed by Cube import owners."""

from __future__ import annotations

from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple

from .models import CubeImportError


def _coerce_symbol(value: Any, context: str) -> str:
    """Coerce a required symbol-like value or raise an import error."""

    symbol = _coerce_str(value)
    if not symbol:
        raise CubeImportError(f"Invalid {context}: {value!r}")
    return symbol


def _coerce_str(value: Any) -> Optional[str]:
    """Coerce primitives into trimmed strings when that preserves intent."""

    if isinstance(value, str):
        cleaned = value.strip()
        return cleaned if cleaned else None
    if isinstance(value, (int, float)):
        return str(value)
    return None


def _coerce_vec2(value: Any) -> Optional[List[float]]:
    """Coerce a two-element sequence into numeric layout coordinates."""

    if isinstance(value, (list, tuple)) and len(value) == 2:
        try:
            return [float(value[0]), float(value[1])]
        except (TypeError, ValueError):
            return None
    return None


def _coerce_vec2_tuple(
    value: Sequence[float] | Tuple[float, float],
) -> Tuple[float, float]:
    """Return a two-float tuple for importer placement calculations."""

    vec = _coerce_vec2(value)
    if vec is None:
        return (0.0, 0.0)
    return (vec[0], vec[1])


def _coerce_execution_mode(value: object) -> Optional[int]:
    """Return a valid non-default LiteGraph execution mode."""

    if isinstance(value, bool) or not isinstance(value, int):
        return None
    if value <= 0:
        return None
    return value


def _coerce_layout_ds(value: Any) -> Dict[str, Any]:
    """Normalize the workflow pan-and-zoom metadata for importer reuse."""

    if isinstance(value, Mapping):
        scale = _coerce_float(value.get("scale"), 1.0)
        offset = _coerce_vec2(value.get("offset")) or [0.0, 0.0]
    else:
        scale = 1.0
        offset = [0.0, 0.0]
    return {"scale": scale, "offset": offset}


def _coerce_float(value: Any, default: float) -> float:
    """Coerce numeric-like values while keeping invalid layout values harmless."""

    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _round_value(value: float, places: int = 3) -> float:
    """Round layout coordinates to a stable precision for frontend payloads."""

    return round(float(value), places)


def _derive_marker_title(alias: str, kind: str) -> str:
    """Derive a readable marker title when layout metadata is absent."""

    suffix = alias.split(".")[-1].replace("_", " ").title()
    if kind:
        return f"{kind.title()} - {suffix}"
    return suffix
