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
"""Compose the self-updater for the pack folder ComfyUI loaded."""

from __future__ import annotations

import os
import time
from pathlib import Path

from .http_client import HttpClient, UrllibHttpClient
from .release_config import (
    ReleaseSourceConfig,
    configured_release_source_path,
    load_release_config,
)
from .release_sources import GitHubReleases, RegistryReleases
from .release_stager import ReleaseStager
from .service import BackgroundRunner, Clock, SelfUpdateService, ThreadRunner


def installed_pack_folder() -> Path:
    """Return the pack folder as ComfyUI reached it, without resolving links.

    Resolving would turn a junction or symbolic link into its target and hide
    that the folder is linked, which the updater must leave alone.
    """

    return Path(os.path.abspath(__file__)).parents[3]


def _wall_clock_ms() -> int:
    """Return milliseconds since the Unix epoch."""

    return time.time_ns() // 1_000_000


def build_self_update_service(
    *,
    pack_folder: Path,
    running_version: str,
    config: ReleaseSourceConfig | None = None,
    http: HttpClient | None = None,
    clock: Clock = _wall_clock_ms,
    runner: BackgroundRunner | None = None,
) -> SelfUpdateService:
    """Build the updater with the shipped release sources and real transport."""

    sources = config or load_release_config(configured_release_source_path())
    transport = http or UrllibHttpClient()
    stager = ReleaseStager(
        pack_folder=pack_folder,
        pack_id=sources.pack_id,
        registry=RegistryReleases(
            pack_id=sources.pack_id, config=sources.registry, http=transport
        ),
        github=GitHubReleases(config=sources.github_release, http=transport),
        http=transport,
    )
    return SelfUpdateService(
        pack_folder=pack_folder,
        running_version=running_version,
        stager=stager,
        clock=clock,
        runner=runner or ThreadRunner(),
    )


__all__ = ["build_self_update_service", "installed_pack_folder"]
