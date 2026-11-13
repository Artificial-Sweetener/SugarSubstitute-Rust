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
"""Translate reviewed discovery evidence into URL-free acquisition selections for Vue."""

from __future__ import annotations

from urllib.parse import quote

from .discovery_plans import ReviewedDiscoveryPlan


def discovery_plan_payload(reviewed: ReviewedDiscoveryPlan) -> dict[str, object]:
    """Preserve every ordered provider offer while exposing only server-owned preview routes."""
    plan = reviewed.plan
    return {
        "schemaVersion": 1,
        "planId": reviewed.plan_id,
        "artifactKind": plan.context.artifact_kind,
        "familyId": plan.context.family_id,
        "publicOnly": reviewed.public_only,
        "browseUrls": [
            {"providerId": provider, "url": url} for provider, url in plan.browse_urls
        ],
        "suggestions": [
            {
                "identity": suggestion.identity,
                "modelName": suggestion.model_name,
                "versionName": suggestion.version_name,
                "creator": suggestion.creator,
                "offers": [
                    {
                        "providerId": offer.reference.provider_id,
                        "providerName": offer.reference.provider_name,
                        "modelId": offer.reference.model_id,
                        "versionId": offer.reference.version_id,
                        "modelPageUrl": offer.model_page_url,
                        "fileName": offer.artifact.file_name,
                        "sizeBytes": offer.artifact.size_bytes,
                        "access": offer.artifact.access.value,
                        "previewUrl": (
                            f"/sugarcubes/v1/models/discovery/{reviewed.plan_id}/preview/{index}/{quote(offer.reference.provider_id, safe='')}"
                            if offer.thumbnail_url is not None
                            else None
                        ),
                    }
                    for offer in suggestion.offers
                ],
            }
            for index, suggestion in enumerate(plan.suggestions)
        ],
    }
