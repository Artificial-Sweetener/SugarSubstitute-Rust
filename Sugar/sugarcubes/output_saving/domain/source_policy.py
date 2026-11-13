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
"""Resolve cube saving from authoritative active workflow order and local mutes."""

from __future__ import annotations

from dataclasses import dataclass

from .preferences import PersistenceMode


@dataclass(frozen=True, slots=True)
class CubeSavingPolicy:
    """Freeze eligible cube identities once per run without retaining host graph objects."""

    persisted_cube_ids: frozenset[str]

    @classmethod
    def create(
        cls,
        *,
        mode: str,
        active_cube_ids: tuple[str, ...],
        muted_cube_ids: frozenset[str],
    ) -> CubeSavingPolicy:
        """Choose final before muting so a muted final never promotes another cube."""
        policy_mode = PersistenceMode(mode)
        if len(set(active_cube_ids)) != len(active_cube_ids) or any(
            not identity for identity in active_cube_ids
        ):
            raise ValueError(
                "Active cube order must contain unique nonempty identities."
            )
        selected = (
            active_cube_ids
            if policy_mode is PersistenceMode.ALL
            else active_cube_ids[-1:]
        )
        return cls(frozenset(selected).difference(muted_cube_ids))

    def saves(self, cube_id: str) -> bool:
        """Return eligibility without affecting graph execution or canvas presentation."""
        return cube_id in self.persisted_cube_ids
