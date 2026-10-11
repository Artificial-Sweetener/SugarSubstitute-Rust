#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#    SPDX-License-Identifier: AGPL-3.0-or-later
"""Verify explicit dependency preparation is deterministic and fail-closed."""

from __future__ import annotations

import pytest

from sugarcubes.execution.errors import CubeQueueError
from sugarcubes.execution.models import ApiPrompt
from sugarcubes.runtime.prompt_preparation import ComfyNodePromptPreparer


def test_prepares_each_present_class_once_and_leaves_unrelated_nodes_alone() -> None:
    """Let declared registered owners add dependencies without generic host hooks."""
    calls: list[ApiPrompt] = []

    class Capture:
        """Declare one trusted host preparation capability."""

        SUGARCUBES_PROMPT_PREPARATION_API_VERSION = 1

        @staticmethod
        def prepare_sugarcubes_prompt(prompt: ApiPrompt) -> None:
            """Install one graph dependency."""
            calls.append(prompt)
            prompt["dependency"] = {"class_type": "Dependency", "inputs": {}}

    registry: dict[str, object] = {"Capture": Capture, "Plain": object}
    prompt: ApiPrompt = {
        "a": {"class_type": "Capture"},
        "b": {"class_type": "Capture"},
        "c": {"class_type": "Plain"},
    }
    ComfyNodePromptPreparer(lambda: registry).prepare(prompt)
    assert calls == [prompt]
    assert "dependency" in prompt


@pytest.mark.parametrize("version", [0, 2])
def test_rejects_unknown_capability_versions(version: int) -> None:
    """Prevent a changed public preparation contract from queueing silently."""

    class Invalid:
        """Declare an unsupported public contract."""

        SUGARCUBES_PROMPT_PREPARATION_API_VERSION = version

    with pytest.raises(CubeQueueError, match="unsupported Cube prompt preparation"):
        ComfyNodePromptPreparer(lambda: {"Invalid": Invalid}).prepare(
            {"node": {"class_type": "Invalid"}}
        )


def test_preparation_failure_retains_owner_and_exception_context() -> None:
    """Report the exact node class and retain the original failure."""

    class Broken:
        """Represent a registered capability whose source cannot be resolved."""

        SUGARCUBES_PROMPT_PREPARATION_API_VERSION = 1

        @staticmethod
        def prepare_sugarcubes_prompt(prompt: ApiPrompt) -> None:
            """Reject an invalid source graph."""
            raise ValueError("Source sampler is missing")

    with pytest.raises(
        CubeQueueError, match="Broken.*Source sampler is missing"
    ) as error:
        ComfyNodePromptPreparer(lambda: {"Broken": Broken}).prepare(
            {"node": {"class_type": "Broken"}}
        )
    assert isinstance(error.value.__cause__, ValueError)
