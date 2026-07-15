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

from __future__ import annotations

from typing import Any
import pytest

from sugarcubes.nodes import CubeInput, CubeOutput
from sugarcubes.runtime import (
    CubeOutputArtifact,
    CubeOutputEvent,
    register_cube_output_observer,
    unregister_cube_output_observer,
)


class RecordingObserver:
    """Record cube output events emitted by a node under test."""

    def __init__(self) -> None:
        self.events: list[CubeOutputEvent] = []

    def on_cube_output(self, event: CubeOutputEvent) -> None:
        """Record one cube output event."""

        self.events.append(event)


class FailingObserver:
    """Raise during cube output delivery."""

    def on_cube_output(self, event: Any) -> None:
        """Fail while handling one cube output event."""

        raise RuntimeError("observer failed")


def test_cube_input_requires_id_and_name() -> None:
    node = CubeInput()
    node.forward("value", "", "Demo")
    with pytest.raises(ValueError):
        node.forward("value", "local/demo", "")


def test_cube_output_requires_id_and_name() -> None:
    node = CubeOutput()
    result = node.forward("value", "", "Demo")
    assert result["result"] == ("value",)
    with pytest.raises(ValueError):
        node.forward("value", "local/demo", "")


def test_cube_output_is_output_node() -> None:
    assert CubeOutput.OUTPUT_NODE is True


def test_marker_nodes_declare_graph_passthrough_outputs() -> None:
    """Marker nodes publish exact value pass-through provenance metadata."""

    assert CubeInput.GRAPH_PASSTHROUGH_OUTPUTS == {0: "value"}
    assert CubeOutput.GRAPH_PASSTHROUGH_OUTPUTS == {0: "value"}


def test_cube_output_emits_event_with_metadata() -> None:
    observer = RecordingObserver()
    register_cube_output_observer(observer)
    try:
        result = CubeOutput().forward(
            "value",
            "owner/repo/demo.cube",
            "Demo",
            "Instance Demo",
            "instance-1",
        )
    finally:
        unregister_cube_output_observer(observer)

    assert result == {"result": ("value",), "ui": {}}
    assert len(observer.events) == 1
    event = observer.events[0]
    assert event.version == 1
    assert event.prompt_id is None
    assert event.node_id is None
    assert event.list_index is None
    assert event.cube_id == "owner/repo/demo.cube"
    assert event.default_alias == "Demo"
    assert event.instance_alias == "Instance Demo"
    assert event.instance_id == "instance-1"
    assert event.media_kind == "value"
    assert event.value_type == "builtins.str"
    assert event.artifacts == ()


def test_cube_output_uses_comfy_execution_context(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    observer = RecordingObserver()
    register_cube_output_observer(observer)
    monkeypatch.setattr(
        "sugarcubes.nodes._current_execution_context",
        lambda: type(
            "Context",
            (),
            {"prompt_id": "prompt-1", "node_id": "node-2", "list_index": 3},
        )(),
    )
    try:
        CubeOutput().forward("value", "owner/repo/demo.cube", "Demo")
    finally:
        unregister_cube_output_observer(observer)

    event = observer.events[0]
    assert event.prompt_id == "prompt-1"
    assert event.node_id == "node-2"
    assert event.list_index == 3


def test_cube_output_image_value_produces_preview_and_artifacts(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifact = CubeOutputArtifact(
        filename="ComfyUI_temp_demo_00001_.png",
        subfolder="",
        type="temp",
        media_kind="image",
        mime_type="image/png",
        width=8,
        height=6,
    )
    monkeypatch.setattr(
        "sugarcubes.nodes._build_output_preview",
        lambda value: (
            {
                "images": [
                    {"filename": artifact.filename, "subfolder": "", "type": "temp"}
                ]
            },
            (artifact,),
        ),
    )
    observer = RecordingObserver()
    register_cube_output_observer(observer)
    try:
        result = CubeOutput().forward(object(), "owner/repo/demo.cube", "Demo")
    finally:
        unregister_cube_output_observer(observer)

    assert result["ui"] == {
        "images": [{"filename": artifact.filename, "subfolder": "", "type": "temp"}]
    }
    event = observer.events[0]
    assert event.media_kind == "image"
    assert event.artifacts == (artifact,)


def test_cube_output_observer_failure_does_not_fail_forward() -> None:
    failing = FailingObserver()
    register_cube_output_observer(failing)
    try:
        result = CubeOutput().forward("value", "owner/repo/demo.cube", "Demo")
    finally:
        unregister_cube_output_observer(failing)

    assert result["result"] == ("value",)
