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
"""Project the shared update feed for browser and sibling-extension consumers."""

from __future__ import annotations

from ..application.model_update_feed import ModelUpdateFeed
from .model_release_payloads import model_release_payload


def model_update_snapshot(feed: ModelUpdateFeed) -> dict[str, object]:
    """Expose fresh presentation values without sharing mutable policy or usage state."""
    return {
        "schemaVersion": 1,
        "enabled": feed.service.usage.load().enabled,
        "proposals": [
            {
                "modelId": item.current.location_id,
                "sha256": item.current.sha256,
                "modelName": item.candidate.version.title,
                "versionId": item.candidate.version.version_id,
                "current": {
                    "providerModelId": item.current.model_id,
                    "versionId": item.current.version_id,
                    "kind": item.current.kind,
                    "baseModel": item.current.base_model,
                    "usageCount": item.current.usage_count,
                    "lastUsedAt": (
                        item.current.last_used_at.isoformat()
                        if item.current.last_used_at
                        else None
                    ),
                },
                "candidate": model_release_payload(
                    item.candidate, item.current.location_id
                ),
            }
            for item in feed.proposals
        ],
    }
