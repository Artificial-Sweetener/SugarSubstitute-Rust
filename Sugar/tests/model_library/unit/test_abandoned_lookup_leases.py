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
"""Prove a lookup interrupted by a killed host is taken up again when the library next starts."""

from __future__ import annotations

from pathlib import Path

from sugarcubes.model_library.domain.models import ModelEvidence
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)

_FILE = ModelEvidence("loras", "a.safetensors", "root", "a.safetensors", 10, 1)


def test_a_lease_left_by_a_killed_host_is_released_at_start(tmp_path: Path) -> None:
    """Reserve again at once after a restart; without the release it would wait out the lease."""
    path = tmp_path / "enrichment.sqlite3"
    attempts = SqliteEnrichmentAttempts(path)
    assert attempts.reserve(_FILE, 1000.0, policy="civitai", providers=("civitai",))
    # The host dies here: the lease is never finished, and it lasts fifteen minutes.
    assert not attempts.reserve(_FILE, 1001.0, policy="civitai", providers=("civitai",))
    restarted = SqliteEnrichmentAttempts(path)
    assert restarted.release_abandoned() == 1
    assert restarted.reserve(_FILE, 1002.0, policy="civitai", providers=("civitai",))


def test_a_finished_lookup_is_not_touched_by_the_release(tmp_path: Path) -> None:
    """Leave completed decisions and their retry deadlines exactly as they were."""
    attempts = SqliteEnrichmentAttempts(tmp_path / "enrichment.sqlite3")
    assert attempts.reserve(_FILE, 1000.0, policy="civitai", providers=("civitai",))
    attempts.finish(
        _FILE, "a" * 64, "failed", 5000.0, policy="civitai", providers=("civitai",)
    )
    assert attempts.release_abandoned() == 0
    assert attempts.failed_lookups() == {_FILE.location_id}
    assert not attempts.reserve(_FILE, 1001.0, policy="civitai", providers=("civitai",))
