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
"""Isolate optional history transport failures from successful metadata publication."""

from __future__ import annotations

import logging
import sqlite3
import aiohttp

from ..application.model_update_feed import ModelUpdateFeed

_LOGGER = logging.getLogger(__name__)


class ModelEnrichmentUpdateCheck:
    """Observe initial metadata while retaining it when history transport or storage fails."""

    def __init__(self, feed: ModelUpdateFeed) -> None:
        """Bind the authoritative update policy without reproducing its daily freshness rules."""
        self._feed = feed

    async def check(self, location_id: str) -> None:
        """Log known adapter failures and leave the next selection eligible to retry."""
        try:
            await self._feed.check_model(location_id)
        except (aiohttp.ClientError, TimeoutError, ValueError, OSError, sqlite3.Error):
            _LOGGER.warning(
                "Initial model update check failed; metadata retained",
                extra={"model_id": location_id},
                exc_info=True,
            )
