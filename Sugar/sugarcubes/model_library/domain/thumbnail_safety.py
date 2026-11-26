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
"""Own the reference CivitAI thumbnail-content policy independently of transport and rendering."""

from __future__ import annotations

from enum import StrEnum


class CivitaiThumbnailSafety(StrEnum):
    """Retain all user-selectable reference policies with stable persistence identities."""

    SFW_ONLY = "sfw_only"
    ALLOW_SOFT = "allow_soft"
    BLUR = "blur"
    ALLOW_ALL = "allow_all"
    DISABLED = "disabled"


# A provider image's content tier: what the strictest policy showing it sharp must allow.
SAFE_TIER = 0
SOFT_TIER = 1
EXPLICIT_TIER = 2


def civitai_image_tier(*, nsfw: bool | None, level: str | int | None) -> int:
    """Classify a provider rating once, so a stored image needs no provider data to be filtered."""
    normalized = level.strip().casefold() if isinstance(level, str) else level
    if nsfw is not True and normalized in (
        None,
        0,
        1,
        "",
        "0",
        "1",
        "none",
        "false",
        "safe",
        "sfw",
    ):
        return SAFE_TIER
    if nsfw is not True and normalized in (2, "2", "soft", "racy"):
        return SOFT_TIER
    return EXPLICIT_TIER


def downloads_tier(policy: CivitaiThumbnailSafety, tier: int) -> bool:
    """Decide whether a policy may fetch an image of this tier at all."""
    if policy is CivitaiThumbnailSafety.DISABLED:
        return False
    if policy in (CivitaiThumbnailSafety.ALLOW_ALL, CivitaiThumbnailSafety.BLUR):
        return True
    return tier <= (
        SOFT_TIER if policy is CivitaiThumbnailSafety.ALLOW_SOFT else SAFE_TIER
    )


def blurs_tier(policy: CivitaiThumbnailSafety, tier: int) -> bool:
    """Decide whether a fetched image of this tier is shown only as its blurred variant."""
    return policy is CivitaiThumbnailSafety.BLUR and tier > SAFE_TIER


def allows_civitai_image(
    policy: CivitaiThumbnailSafety,
    *,
    image_type: str | None,
    nsfw: bool | None,
    level: str | int | None,
) -> bool:
    """Apply configured content restrictions equally to image and video thumbnail sources."""
    if image_type is not None and image_type.casefold() not in ("image", "video"):
        return False
    return downloads_tier(policy, civitai_image_tier(nsfw=nsfw, level=level))


def former_policy_tier(policy: str) -> int:
    """Bound the tier of an image by the setting it was fetched under, before tiers were stored.

    A decision names the setting in force when it was made. An image fetched under a strict
    setting is at most that strict; one fetched under any other setting is treated as explicit
    until its provider offer says otherwise.
    """
    if policy.endswith(":sfw_only"):
        return SAFE_TIER
    if policy.endswith(":allow_soft"):
        return SOFT_TIER
    return EXPLICIT_TIER
