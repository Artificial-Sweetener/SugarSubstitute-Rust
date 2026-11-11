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
"""Prove graph-only draft identity survives workflow analysis and normalization."""

from __future__ import annotations

from copy import deepcopy
from typing import cast

import pytest

from sugarcubes.authoring import NativeWorkflowNormalizer
from sugarcubes.cube_model import CubeDocument
from sugarcubes.workflow import CanonicalWorkflowError, read_canonical_workflow
from sugarcubes.workflow_analysis import CubeGraphAnalysisService
from tests.workflow.support.workflow_fixtures import draft_workflow


class _NoCatalog:
    """Fail if a graph-only draft tries to resolve an installed Cube."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Reject catalog lookup for an unsaved graph."""

        raise AssertionError((cube_id, version_pin))


@pytest.mark.parametrize("omit_identity", [False, True])
def test_draft_analysis_preserves_exact_native_prompt_and_empty_identity(
    omit_identity: bool,
) -> None:
    """Analyze drafts with absent or empty saved identity without fabricating one."""

    source = draft_workflow()
    if omit_identity:
        definition = cast(
            dict[str, object],
            cast(
                list[object],
                cast(dict[str, object], source["definitions"])["subgraphs"],
            )[0],
        )
        metadata = cast(
            dict[str, object],
            cast(dict[str, object], definition["extra"])["sugarcubes_cube"],
        )
        metadata.pop("cube_id")
        metadata.pop("cube_version")
    original = deepcopy(source)
    analysis = CubeGraphAnalysisService(NativeWorkflowNormalizer(_NoCatalog())).analyze(
        source
    )

    assert analysis.normalized_workflow == original
    assert source == original
    assert len(analysis.instances) == 1
    assert analysis.instances[0].cube_id == ""
    assert analysis.instances[0].cube_version == ""
    assert analysis.instances[0].is_draft
    canonical = read_canonical_workflow(source)
    assert canonical.definitions[0].is_draft
    assert canonical.definitions[0].document is None


@pytest.mark.parametrize(
    "field,value",
    [
        ("cube_id", "local/personal/pretend.cube"),
        ("cube_version", "1.0.0"),
        ("cube_id", 7),
    ],
)
def test_draft_rejects_a_saved_or_malformed_identity(field: str, value: object) -> None:
    """Require workflow-only drafts to remain distinct from saved definitions."""

    source = draft_workflow()
    definition = cast(
        dict[str, object],
        cast(list[object], cast(dict[str, object], source["definitions"])["subgraphs"])[
            0
        ],
    )
    metadata = cast(
        dict[str, object],
        cast(dict[str, object], definition["extra"])["sugarcubes_cube"],
    )
    metadata[field] = value
    with pytest.raises(CanonicalWorkflowError, match="draft"):
        read_canonical_workflow(source)


def test_instance_and_definition_must_agree_on_draft_state() -> None:
    """Reject a saved instance that points at an unsaved draft definition."""

    source = draft_workflow()
    instance = cast(dict[str, object], cast(list[object], source["nodes"])[0])
    cast(dict[str, object], instance["properties"])["sugarcubes_kind"] = "cube"
    with pytest.raises(CanonicalWorkflowError, match="draft"):
        read_canonical_workflow(source)
