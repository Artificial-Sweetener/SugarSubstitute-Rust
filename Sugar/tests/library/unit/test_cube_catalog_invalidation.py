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
"""Prove scoped catalog invalidation preserves unrelated artifact hashes."""

from __future__ import annotations

from collections.abc import Mapping

from sugarcubes.backend.services.cube_catalog_state_service import (
    CubeCatalogStateService,
)


def test_targeted_invalidation_rebuilds_only_the_named_cube() -> None:
    """Retain unrelated rows even when the caller explicitly invalidates equal stat facts."""

    summaries: tuple[Mapping[str, object], ...] = (
        {"cube_id": "a", "path": "a.cube", "mtime_ns": 1},
        {"cube_id": "b", "path": "b.cube", "mtime_ns": 1},
    )
    rebuilt: list[str] = []

    def build_entry(summary: Mapping[str, object]) -> dict[str, object]:
        """Count artifact hash reads without filesystem effects."""

        cube_id = str(summary["cube_id"])
        rebuilt.append(cube_id)
        return {"cubeId": cube_id, "contentHash": cube_id}

    service = CubeCatalogStateService(
        list_summaries=lambda _: summaries,
        build_entry=build_entry,
        revision_pack_facts=lambda _: [],
        pack_counts=lambda: {},
        generated_at=lambda: "now",
    )
    initial = service.current_revision()
    service.invalidate("metadata_changed", affected_cube_ids=["a"])
    assert service.current_revision() == initial
    assert rebuilt == ["a", "b", "a"]
    service.invalidate("full_reset")
    service.current_revision()
    assert rebuilt == ["a", "b", "a", "a", "b"]
