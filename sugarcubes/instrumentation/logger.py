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
"""Structured logging utilities for SugarCubes."""

from __future__ import annotations

import json
import logging
import os
from typing import Any, Mapping

_logger = logging.getLogger("sugarcubes.events")
_DIAGNOSTICS_ENV_VAR = "SUGARCUBES_DIAGNOSTICS"
_ENABLED_VALUES = {"1", "true", "yes", "on"}


def _normalize_log_value(value: Any) -> Any:
    """Normalize a log payload value into JSON-safe data."""

    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, Mapping):
        return {str(key): _normalize_log_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set, frozenset)):
        return [_normalize_log_value(item) for item in value]
    return repr(value)


def _normalize_log_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize a log payload mapping for structured emission."""

    return {str(key): _normalize_log_value(value) for key, value in payload.items()}


def diagnostics_enabled() -> bool:
    """Return whether SugarCubes diagnostic logs should be promoted to INFO."""

    return (
        os.environ.get(_DIAGNOSTICS_ENV_VAR, "").strip().casefold() in _ENABLED_VALUES
    )


def diagnostic_log_level() -> int:
    """Return the active log level for normal-operation diagnostic logs."""

    return logging.INFO if diagnostics_enabled() else logging.DEBUG


def log_diagnostic(
    logger: logging.Logger,
    marker: str,
    event: str,
    fields: Mapping[str, object],
) -> None:
    """Emit a marker-style diagnostic line using the shared diagnostics policy."""

    normalized_fields = _normalize_log_payload(fields)
    segments = [str(marker), f"event={event}"]
    segments.extend(
        f"{key}={value}" for key, value in sorted(normalized_fields.items())
    )
    logger.log(diagnostic_log_level(), " ".join(segments))


def log_event(phase: str, event: str, payload: Mapping[str, Any]) -> None:
    """Emit a structured SugarCubes event log line."""

    record = {
        "phase": str(phase),
        "event": str(event),
        "payload": _normalize_log_payload(payload),
    }
    _logger.log(
        diagnostic_log_level(),
        "sugarcubes.event %s",
        json.dumps(record, sort_keys=True),
    )
