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
"""Prove native submissions are managed before writes and fail closed on policy errors."""

from copy import deepcopy
from pathlib import Path
import pytest

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.execution.coordinator import CubeExecutionCoordinator
from sugarcubes.execution.models import CubeExecutionRequest
from tests.execution.support.execution_fixtures import cube_workflow, provider_document

from sugarcubes.output_saving.infrastructure.native_prompt_hook import (
    NativeSavingPromptHook,
)
from sugarcubes.output_saving.infrastructure.policy_error_node import (
    OutputSavingPolicyError,
)
from sugarcubes.output_saving.infrastructure.runtime import OutputSavingRuntime


def test_native_payload_preserves_fields_and_assigns_a_fresh_server_identity(
    tmp_path: Path,
) -> None:
    """Leave ordinary native save inputs untouched while management is off by default."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    hook = NativeSavingPromptHook(runtime.policy)
    original = {
        "prompt": {
            "save": {
                "class_type": "SaveImage",
                "inputs": {"images": ["image", 0], "filename_prefix": "Original"},
            }
        },
        "client_id": "browser",
        "extra_data": {"private_client_value": "retained"},
    }
    captured = deepcopy(original)
    prepared = hook(original)
    assert isinstance(prepared, dict)
    assert isinstance(prepared["prompt_id"], str)
    assert prepared["prompt"] == original["prompt"]
    assert prepared["client_id"] == "browser"
    assert prepared["extra_data"] == {"private_client_value": "retained"}
    assert original == captured
    assert not (tmp_path / "output").exists()


def test_native_policy_error_replaces_execution_candidate_with_validation_failure(
    tmp_path: Path,
) -> None:
    """Prevent original save nodes from writing after required settings could not be loaded."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    (tmp_path / "state" / "preferences.json").write_text(
        "invalid settings", encoding="utf-8"
    )
    original = {
        "prompt": {
            "save": {"class_type": "SaveImage", "inputs": {"images": ["image", 0]}}
        }
    }
    prepared = NativeSavingPromptHook(runtime.policy)(original)
    assert isinstance(prepared, dict)
    prompt = prepared["prompt"]
    assert isinstance(prompt, dict)
    assert "save" not in prompt
    error = prompt["__sugarcubes_output_saving_error__"]
    assert error["class_type"] == "SugarCubes.OutputSavingPolicyError"
    assert isinstance(
        OutputSavingPolicyError.VALIDATE_INPUTS(error["inputs"]["message"]), str
    )
    assert not (tmp_path / "output").exists()


def test_native_hook_does_not_retain_active_behavior_after_shutdown(
    tmp_path: Path,
) -> None:
    """Return the exact original object when a retired handler remains in the host list."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    hook = NativeSavingPromptHook(runtime.policy)
    hook.close()
    payload: dict[str, object] = {"prompt": {}}
    assert hook(payload) is payload


def test_unexpected_host_policy_exception_cannot_fall_back_to_original_native_savers(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    """Translate arbitrary adapter failures into rejection before Comfy's fail-open hook loop sees them."""

    class ExternalFailure(Exception):
        """Model an unexpected custom-host adapter error outside ordinary exception families."""

    def reject_policy(*args: object, **kwargs: object) -> ApiPrompt:
        """Fail before any original node may be executed or write files."""
        raise ExternalFailure("fixture unexpected policy failure")

    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    monkeypatch.setattr(runtime.policy, "prepare", reject_policy)
    prepared = NativeSavingPromptHook(runtime.policy)(
        {
            "prompt": {
                "save": {"class_type": "SaveImage", "inputs": {"images": ["image", 0]}}
            }
        }
    )
    assert isinstance(prepared, dict)
    prompt = prepared["prompt"]
    assert isinstance(prompt, dict)
    assert "save" not in prompt
    assert (
        prompt["__sugarcubes_output_saving_error__"]["class_type"]
        == "SugarCubes.OutputSavingPolicyError"
    )
    assert "rejected the request before any node execution" in caplog.text


@pytest.mark.parametrize("direct", [False, True])
def test_queue_captures_the_named_recipe_or_direct_context_without_a_mounted_canvas(
    tmp_path: Path, direct: bool
) -> None:
    """Capture the correct native/direct workflow name once and retain it independently of later edits."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    workflow = cube_workflow({"a": provider_document("A")})
    workflow["extra"] = {"sugarcubes_output_saving": {"workflow_name": "Recipe name"}}
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )
    prompt = runtime.policy.prepare(
        "named-run",
        prepared.prompt,
        workflow=workflow,
        owners=prepared.node_owners,
        extra_data=(
            {"sugarcubes_output_saving": {"workflow_name": "Direct name"}}
            if direct
            else {}
        ),
    )
    sink = next(
        node
        for node in prompt.values()
        if node["class_type"] == "SugarCubes.CubeOutput"
    )
    inputs = sink["inputs"]
    assert isinstance(inputs, dict) and isinstance(inputs["save_ticket"], str)
    ticket = runtime.journal.read_ticket(inputs["save_ticket"])
    assert ticket.run.workflow == ("Direct name" if direct else "Recipe name")
    workflow["extra"] = {"sugarcubes_output_saving": {"workflow_name": "Later name"}}
    assert (
        runtime.journal.read_ticket(inputs["save_ticket"]).run.workflow
        == ticket.run.workflow
    )


@pytest.mark.parametrize(
    "class_type",
    [
        "SugarCubes.ManagedImageOutput",
        "SugarCubes.OutputSavingPolicyError",
        "SugarCubes.CubeOutput",
    ],
)
def test_authored_private_sink_or_replayed_ticket_is_rejected_before_execution(
    tmp_path: Path, class_type: str
) -> None:
    """Prevent a replayed execution prompt from reusing cached artifacts or another run's saving authority."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    result = NativeSavingPromptHook(runtime.policy)(
        {
            "prompt": {
                "replayed": {
                    "class_type": class_type,
                    "inputs": {"save_ticket": "previous-run-ticket"},
                }
            }
        }
    )
    assert isinstance(result, dict)
    prompt = result["prompt"]
    assert isinstance(prompt, dict) and "replayed" not in prompt
    assert (
        "Execution-only saving nodes"
        in prompt["__sugarcubes_output_saving_error__"]["inputs"]["message"]
    )
    assert not (tmp_path / "output").exists()
