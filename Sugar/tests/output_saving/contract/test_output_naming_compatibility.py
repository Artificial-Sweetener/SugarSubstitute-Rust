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
"""Prove Substitute-compatible path tokens and safe ordinary-source extensions."""

from datetime import datetime

import pytest

from sugarcubes.output_saving.domain.path_pattern import (
    DEFAULT_PATH_PATTERN,
    OutputNamingContext,
    render_components,
    run_bucket_pattern,
    validate_pattern,
)


def test_default_pattern_matches_substitute_padding_and_case_rules() -> None:
    """Preserve user-visible folder names while normalizing filename tokens."""
    context = OutputNamingContext(
        workflow="My Workflow",
        source="Main Output",
        cube="Text To Image",
        started_at=datetime(2026, 5, 1, 14, 32, 9),
        run=7,
        cube_number=1,
        folder_image_number=9,
        seed="18319146648460766796",
        width=1216,
        height=1536,
        index=3,
        set_index=2,
    )
    assert render_components(DEFAULT_PATH_PATTERN, context) == (
        "2026-05-01",
        "007_01_my_workflow_main_output",
    )
    assert render_components(
        "{workflow}/{cube}/{image#}_{seed}_{width}_{height}_{index}_{set}_{time}_{day}",
        context,
    ) == (
        "My Workflow",
        "Text To Image",
        "09_18319146648460766796_1216_1536_3_2_14-32-09_friday",
    )


def test_ordinary_source_tokens_preserve_exact_node_identity_and_batch_index() -> None:
    """Extend naming without changing the meaning of Substitute's existing tokens."""
    context = OutputNamingContext(
        workflow="Workflow",
        source="Save Image",
        node="Result",
        node_id="42:7",
        started_at=datetime(2026, 5, 1),
        index=3,
    )
    assert render_components("{node}_{node#}_{batch}_{index}", context) == (
        "result_42_7_3_3",
    )


def test_dynamic_source_text_cannot_escape_a_component() -> None:
    """Treat slash, markup-like text, device names and traversal as literal labels."""
    context = OutputNamingContext(
        workflow="../<My Workflow>", source="../CON", started_at=datetime(2026, 5, 1)
    )
    assert render_components("{workflow}/{source}", context) == ("My Workflow", "_con")


@pytest.mark.parametrize(
    "pattern",
    [
        "",
        "../image",
        "folder/../image",
        "./image",
        "/image",
        "E:\\image",
        "\\\\server\\image",
        "{missing}",
        "{workflow",
        "folder/",
        "a\x00/b",
    ],
)
def test_invalid_patterns_fail_before_writing(pattern: str) -> None:
    """Reject malformed or escaping configuration rather than repairing it silently."""
    with pytest.raises(ValueError):
        validate_pattern(pattern)


@pytest.mark.parametrize(
    ("pattern", "bucket"),
    [
        (DEFAULT_PATH_PATTERN, "{date}"),
        ("{date}/{run}/{source}", "{date}"),
        ("{workflow}/{date}/{source}", "{workflow}/{date}"),
        ("{run}_{source}", None),
    ],
)
def test_run_buckets_match_substitute(pattern: str, bucket: str | None) -> None:
    """Allocate run numbers in the same folder namespace as the migrated template."""
    assert run_bucket_pattern(pattern) == bucket
