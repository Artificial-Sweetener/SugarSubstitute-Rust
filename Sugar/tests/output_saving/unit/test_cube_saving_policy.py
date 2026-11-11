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
"""Prove active workflow order and local mute semantics independently of canvases."""

import pytest

from sugarcubes.output_saving.domain.source_policy import CubeSavingPolicy


@pytest.mark.parametrize("mode", ["all", "final_cube"])
def test_muted_final_cube_never_promotes_an_earlier_cube(mode: str) -> None:
    """Resolve the final cube before applying the workflow-local saving mute."""
    policy = CubeSavingPolicy.create(
        mode=mode,
        active_cube_ids=("a", "b", "c"),
        muted_cube_ids=frozenset({"c"}),
    )
    assert not policy.saves("c")
    assert policy.saves("a") is (mode == "all")
    assert policy.saves("b") is (mode == "all")
    assert not policy.saves("missing")


def test_final_cube_uses_supplied_workflow_order_and_no_mounted_view() -> None:
    """Ignore identifier sort order and preserve the caller's authoritative stack order."""
    policy = CubeSavingPolicy.create(
        mode="final_cube",
        active_cube_ids=("z", "c", "a"),
        muted_cube_ids=frozenset(),
    )
    assert policy.saves("a")
    assert not policy.saves("z")
    assert not policy.saves("c")


def test_empty_active_stack_has_no_persisted_cube() -> None:
    """Keep disabled or bypassed-only workflows from inventing a final source."""
    policy = CubeSavingPolicy.create(
        mode="final_cube", active_cube_ids=(), muted_cube_ids=frozenset()
    )
    assert not policy.saves("a")


def test_invalid_modes_and_duplicate_identities_are_rejected() -> None:
    """Require an unambiguous ordered active inventory at the policy boundary."""
    with pytest.raises(ValueError):
        CubeSavingPolicy.create(
            mode="last_unmuted", active_cube_ids=("a",), muted_cube_ids=frozenset()
        )
    with pytest.raises(ValueError):
        CubeSavingPolicy.create(
            mode="all", active_cube_ids=("a", "a"), muted_cube_ids=frozenset()
        )
