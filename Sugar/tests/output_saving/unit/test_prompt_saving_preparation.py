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
"""Prove immutable saving policy and execution-only sink rewrites."""

from copy import deepcopy
from collections.abc import Mapping

from sugarcubes.output_saving.domain.saver_contract import ImageSaverContract
from dataclasses import replace
from datetime import datetime
import pytest

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.output_saving.application.prompt_preparer import (
    OutputSavingPromptPreparer,
)
from sugarcubes.output_saving.domain.preferences import (
    OutputSavingPreferences,
    PersistenceMode,
)
from sugarcubes.output_saving.domain.run_plan import SavingRun, SavingSource, SourceKind


class TicketStore:
    """Retain immutable plans at an external persistence boundary for policy assertions."""

    def __init__(self) -> None:
        """Create isolated ticket and run records for each test."""
        self.runs: list[SavingRun] = []
        self.sources: list[SavingSource] = []

    def issue_run(
        self, run: SavingRun, sources: tuple[SavingSource, ...]
    ) -> tuple[str, ...]:
        """Capture the source transaction and return distinguishable opaque identities."""
        self.runs.append(run)
        self.sources.extend(sources)
        return tuple(f"{run.run_id}-{index}" for index in range(1, len(sources) + 1))


def _run(preferences: OutputSavingPreferences) -> SavingRun:
    """Supply a trusted host root and exact run facts without an active canvas."""
    return SavingRun(
        run_id="run-a",
        root="E:/Outputs",
        workflow="Example",
        started_at=datetime(2026, 5, 1),
        preferences=preferences,
        metadata=(("workflow", '{"nodes":[]}'),),
    )


def test_cube_policy_uses_authoritative_order_before_muting_final() -> None:
    """Preserve every cube execution sink while suppressing durable writes for a muted final."""
    prompt: ApiPrompt = {
        "sampler": {
            "class_type": "Sampler",
            "inputs": {"seed": "18319146648460766796"},
        },
        "sink-z": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"value": ["sampler", 0], "save_outputs": True},
        },
        "sink-a": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"value": ["sampler", 0], "save_outputs": False},
        },
    }
    sources = (
        SavingSource("sink-z", "Z", SourceKind.CUBE, cube_id="z", cube_number=1),
        SavingSource(
            "sink-a", "A", SourceKind.CUBE, cube_id="a", cube_number=2, enabled=False
        ),
    )
    original = deepcopy(prompt)
    store = TicketStore()
    prepared = OutputSavingPromptPreparer(store).prepare(
        prompt,
        contracts=_contracts(prompt),
        run=_run(
            replace(
                OutputSavingPreferences(), persistence_mode=PersistenceMode.FINAL_CUBE
            )
        ),
        sources=sources,
        active_cube_ids=("z", "a"),
    )
    assert prompt == original
    assert prepared["sampler"] == prompt["sampler"]
    assert len(store.sources) == 2
    assert all(not source.persist for source in store.sources)
    for source in sources:
        inputs = prepared[source.node_id]["inputs"]
        assert isinstance(inputs, dict)
        assert "save_ticket" in inputs


def test_management_off_preserves_ordinary_nodes_and_unknown_savers() -> None:
    """Keep the built-in cube scope independent of the optional ordinary workflow scope."""
    prompt: ApiPrompt = {
        "save": {
            "class_type": "SaveImage",
            "inputs": {"images": ["sampler", 0], "filename_prefix": "Original"},
        },
        "preview": {"class_type": "PreviewImage", "inputs": {"images": ["sampler", 0]}},
        "custom": {
            "class_type": "UnrecognizedSaver",
            "inputs": {"images": ["sampler", 0]},
        },
    }
    sources = (
        SavingSource("save", "Save", SourceKind.SAVE),
        SavingSource("preview", "Preview", SourceKind.PREVIEW),
    )
    store = TicketStore()
    assert (
        OutputSavingPromptPreparer(store).prepare(
            prompt,
            contracts=_contracts(prompt),
            run=_run(OutputSavingPreferences()),
            sources=sources,
            active_cube_ids=(),
        )
        == prompt
    )
    assert store.runs == []
    assert store.sources == []


def test_managed_save_rewrite_retains_ids_metadata_and_tensor_connections() -> None:
    """Replace only the supported sink's execution policy while preserving the user's workflow graph."""
    prompt: ApiPrompt = {
        "save": {
            "class_type": "SaveImage",
            "inputs": {"images": ["sampler", 0], "filename_prefix": "Original"},
            "_meta": {"title": "Result"},
        },
        "consumer": {"class_type": "Consumer", "inputs": {"image": ["save", 0]}},
    }
    original = deepcopy(prompt)
    store = TicketStore()
    run = _run(replace(OutputSavingPreferences(), manage_workflow=True))
    prepared = OutputSavingPromptPreparer(store).prepare(
        prompt,
        contracts=_contracts(prompt),
        run=run,
        sources=(SavingSource("save", "Result", SourceKind.SAVE),),
        active_cube_ids=(),
    )
    assert prompt == original
    assert prepared["save"]["class_type"] == "SugarCubes.ManagedImageOutput"
    assert prepared["save"]["_meta"] == prompt["save"]["_meta"]
    assert prepared["consumer"] == {
        "class_type": "Consumer",
        "inputs": {"image": ["sampler", 0]},
    }
    assert prepared["save"]["inputs"] == {
        "images": ["sampler", 0],
        "save_ticket": "run-a-1",
    }
    assert store.sources[0].persist
    assert store.runs == [run]


def test_internal_cube_saves_are_transient_even_with_workflow_management_off() -> None:
    """Let public cube outputs own durable writes without double-saving inside supported cubes."""
    prompt: ApiPrompt = {
        "internal": {
            "class_type": "SaveImage",
            "inputs": {"images": ["sampler", 0], "filename_prefix": "Original"},
        }
    }
    store = TicketStore()
    prepared = OutputSavingPromptPreparer(store).prepare(
        prompt,
        contracts=_contracts(prompt),
        run=_run(OutputSavingPreferences()),
        sources=(
            SavingSource("internal", "Internal", SourceKind.INTERNAL, cube_id="a"),
        ),
        active_cube_ids=("a",),
    )
    assert prepared["internal"]["class_type"] == "SugarCubes.ManagedImageOutput"
    assert not store.sources[0].persist


def test_completed_preview_saving_requires_its_separate_opt_in() -> None:
    """Keep completed Preview Image distinct from live sampler frames and ordinary Save Image."""
    prompt: ApiPrompt = {
        "preview": {"class_type": "PreviewImage", "inputs": {"images": ["sampler", 0]}}
    }
    source = SavingSource("preview", "Preview", SourceKind.PREVIEW)
    for enabled in (False, True):
        store = TicketStore()
        prepared = OutputSavingPromptPreparer(store).prepare(
            prompt,
            contracts=_contracts(prompt),
            run=_run(
                replace(
                    OutputSavingPreferences(),
                    manage_workflow=True,
                    save_previews=enabled,
                )
            ),
            sources=(source,),
            active_cube_ids=(),
        )
        assert bool(store.sources) is enabled
        assert (prepared != prompt) is enabled


def _contracts(prompt: ApiPrompt) -> Mapping[str, ImageSaverContract]:
    """Declare only the fixture's known built-ins at the host qualification boundary."""
    kinds = {
        "SugarCubes.CubeOutput": SourceKind.CUBE,
        "SaveImage": SourceKind.SAVE,
        "PreviewImage": SourceKind.PREVIEW,
    }
    result: dict[str, ImageSaverContract] = {}
    for identity, node in prompt.items():
        name = node.get("class_type")
        if isinstance(name, str) and name in kinds:
            result[identity] = ImageSaverContract(
                name,
                "value" if kinds[name] is SourceKind.CUBE else "images",
                kinds[name],
            )
    return result


def test_qualified_custom_saver_delegates_writes_and_preserves_image_consumers() -> (
    None
):
    """Normalize the declared input and bypass only a certified unchanged image result."""
    prompt: ApiPrompt = {
        "custom": {
            "class_type": "AuthorSaver",
            "inputs": {"picture": ["sampler", 0], "prefix": "native"},
        },
        "consumer": {"class_type": "Processor", "inputs": {"image": ["custom", 0]}},
    }
    original = deepcopy(prompt)
    store = TicketStore()
    prepared = OutputSavingPromptPreparer(store).prepare(
        prompt,
        run=_run(replace(OutputSavingPreferences(), manage_workflow=True)),
        sources=(SavingSource("custom", "Custom", SourceKind.SAVE),),
        active_cube_ids=(),
        contracts={
            "custom": ImageSaverContract("AuthorSaver", "picture", SourceKind.SAVE)
        },
    )
    assert prompt == original
    assert prepared["custom"]["inputs"] == {
        "images": ["sampler", 0],
        "save_ticket": "run-a-1",
    }
    assert prepared["consumer"]["inputs"] == {"image": ["sampler", 0]}
    assert store.sources[0].persist


def test_stale_custom_qualification_rejects_before_ticket_transaction() -> None:
    """Never rewrite a class that changed between classification and execution preparation."""
    store = TicketStore()
    with pytest.raises(ValueError, match="qualified node class"):
        OutputSavingPromptPreparer(store).prepare(
            {
                "custom": {
                    "class_type": "OtherSaver",
                    "inputs": {"picture": ["sampler", 0]},
                }
            },
            run=_run(replace(OutputSavingPreferences(), manage_workflow=True)),
            sources=(SavingSource("custom", "Custom", SourceKind.SAVE),),
            active_cube_ids=(),
            contracts={
                "custom": ImageSaverContract("AuthorSaver", "picture", SourceKind.SAVE)
            },
        )
    assert not store.sources
