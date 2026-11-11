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
"""Tests for unique selectable cube-version projection."""

from sugarcubes.backend.services.cube_revision_version_projection import (
    project_unique_cube_versions,
)


def test_projection_keeps_first_revision_for_each_version() -> None:
    """Keep working-tree and newest-commit precedence without rewriting history."""

    projection = project_unique_cube_versions(
        [
            {"revision_ref": "WORKTREE", "version": "1.1.0"},
            {"revision_ref": "newest", "version": "1.0.0"},
            {"revision_ref": "oldest", "version": "1.0.0"},
        ]
    )

    assert [entry["revision_ref"] for entry in projection.revisions] == [
        "WORKTREE",
        "newest",
    ]
    assert projection.omissions == (
        {
            "version": "1.0.0",
            "selected_revision_ref": "newest",
            "omitted_revision_ref": "oldest",
        },
    )


def test_projection_omits_unversioned_history_from_version_choices() -> None:
    """Exclude records that cannot represent a selectable cube version."""

    projection = project_unique_cube_versions(
        [
            {"revision_ref": "unversioned", "version": ""},
            {"revision_ref": "versioned", "version": "2.0.0"},
        ]
    )

    assert projection.revisions == ({"revision_ref": "versioned", "version": "2.0.0"},)
    assert projection.omissions == ()
