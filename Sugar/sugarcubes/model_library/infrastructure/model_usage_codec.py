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
"""Validate durable usage state before it crosses into update policy."""

from __future__ import annotations

from datetime import UTC, datetime
import json
import re

from ..domain.model_updates import ModelUpdateState, ModelUsageRecord


def encode_usage(state: ModelUpdateState) -> str:
    """Serialize explicit consent and artifact-scoped decisions without credentials."""
    return json.dumps(
        {
            "schemaVersion": 1,
            "enabled": state.enabled,
            "records": [
                {
                    "sha256": record.sha256,
                    "locationId": record.location_id,
                    "kind": record.kind,
                    "modelId": record.model_id,
                    "versionId": record.version_id,
                    "baseModel": record.base_model,
                    "usageCount": record.usage_count,
                    "lastUsedAt": (
                        record.last_used_at.isoformat() if record.last_used_at else None
                    ),
                    "dismissedVersionId": record.dismissed_version_id,
                    "updatesDisabled": record.updates_disabled_for_model,
                }
                for record in state.records
            ],
        },
        separators=(",", ":"),
    )


def decode_usage(payload: str) -> ModelUpdateState:
    """Fail closed on malformed consent, usage identities or persistence versions."""
    value: object = json.loads(payload)
    if not isinstance(value, dict) or value.get("schemaVersion") != 1:
        raise ValueError("Invalid model update state schema.")
    enabled, rows = value.get("enabled"), value.get("records")
    if not isinstance(enabled, bool) or not isinstance(rows, list):
        raise ValueError("Invalid model update consent or records.")
    records = tuple(_record(row) for row in rows)
    if len({record.sha256 for record in records}) != len(records):
        raise ValueError("Duplicate model usage identities.")
    return ModelUpdateState(enabled, records)


def _record(value: object) -> ModelUsageRecord:
    """Require exact content identities and aware timestamps for relevance calculations."""
    if not isinstance(value, dict):
        raise ValueError("Invalid model usage record.")
    sha256, location = _text(value.get("sha256")), _text(value.get("locationId"))
    if not re.fullmatch(r"[a-f0-9]{64}", sha256) or not re.fullmatch(
        r"[a-f0-9]{64}", location
    ):
        raise ValueError("Invalid model usage identity.")
    count, disabled = value.get("usageCount"), value.get("updatesDisabled")
    if (
        isinstance(count, bool)
        or not isinstance(count, int)
        or count < 0
        or not isinstance(disabled, bool)
    ):
        raise ValueError("Invalid model usage count or opt-out.")
    timestamp = (
        datetime.fromisoformat(_text(value.get("lastUsedAt")))
        if value.get("lastUsedAt") is not None
        else None
    )
    if (timestamp is None) != (count == 0):
        raise ValueError("Invalid model usage count or timestamp.")
    if timestamp is not None and timestamp.tzinfo is None:
        raise ValueError("Model usage timestamps must be timezone-aware.")
    return ModelUsageRecord(
        sha256,
        location,
        _text(value.get("kind")),
        _identity(value.get("modelId")),
        _identity(value.get("versionId")),
        _text(value.get("baseModel")),
        count,
        timestamp.astimezone(UTC) if timestamp else None,
        _identity(value.get("dismissedVersionId")),
        disabled,
    )


def _text(value: object) -> str:
    """Preserve typed persisted text rather than coercing corrupt input."""
    if not isinstance(value, str):
        raise ValueError("Invalid model usage text.")
    return value


def _identity(value: object) -> str:
    """Allow absent provider evidence while rejecting malformed known identifiers."""
    text = _text(value)
    if text and not re.fullmatch(r"[1-9][0-9]{0,15}", text):
        raise ValueError("Invalid model usage provider identity.")
    return text
