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
import importlib.util
import logging
import sys
from pathlib import Path

from sugarcubes.runtime import (
    CUBE_OUTPUT_OBSERVER_API_VERSION,
    CubeOutputEvent,
    register_cube_output_observer,
    unregister_cube_output_observer,
)
from sugarcubes.runtime.cube_output_events import notify_cube_output_observers


class RecordingObserver:
    """Record cube output events delivered by the registry."""

    def __init__(self) -> None:
        self.events: list[CubeOutputEvent] = []

    def on_cube_output(self, event: Any) -> None:
        """Record one delivered event."""

        self.events.append(event)


class FailingObserver:
    """Raise during delivery to verify registry isolation."""

    def on_cube_output(self, event: Any) -> None:
        """Fail while handling one event."""

        raise RuntimeError("delivery failed")


def make_event() -> Any:
    """Build a minimal cube output event for registry tests."""

    return CubeOutputEvent(
        version=1,
        prompt_id="prompt-1",
        node_id="node-1",
        list_index=0,
        cube_id="owner/repo/demo.cube",
        default_alias="Demo",
        instance_alias="Demo",
        instance_id="instance-1",
        media_kind="value",
        value_type="builtins.str",
        artifacts=(),
    )


def test_registering_observer_receives_event() -> None:
    observer = RecordingObserver()
    event = make_event()
    register_cube_output_observer(observer)
    try:
        notify_cube_output_observers(event)
    finally:
        unregister_cube_output_observer(observer)

    assert observer.events == [event]


def test_runtime_event_bus_is_shared_across_import_identities() -> None:
    """Observer state should stay canonical when Comfy imports runtime under another name."""

    first = load_cube_output_events_module("sugarcubes_test_runtime_first")
    second = load_cube_output_events_module("sugarcubes_test_runtime_second")
    observer = RecordingObserver()
    event = second.CubeOutputEvent(
        version=1,
        prompt_id="prompt-1",
        node_id="node-1",
        list_index=0,
        cube_id="owner/repo/demo.cube",
        default_alias="Demo",
        instance_alias="Demo",
        instance_id="instance-1",
        media_kind="value",
        value_type="builtins.str",
        artifacts=(),
    )

    first.register_cube_output_observer(observer)
    try:
        second.notify_cube_output_observers(event)
    finally:
        first.unregister_cube_output_observer(observer)

    assert observer.events == [event]
    assert first.CUBE_OUTPUT_OBSERVER_API_VERSION == CUBE_OUTPUT_OBSERVER_API_VERSION


def test_unregistering_observer_stops_delivery() -> None:
    observer = RecordingObserver()
    register_cube_output_observer(observer)
    unregister_cube_output_observer(observer)

    notify_cube_output_observers(make_event())

    assert observer.events == []


def test_duplicate_registration_is_ignored() -> None:
    observer = RecordingObserver()
    event = make_event()
    register_cube_output_observer(observer)
    register_cube_output_observer(observer)
    try:
        notify_cube_output_observers(event)
    finally:
        unregister_cube_output_observer(observer)

    assert observer.events == [event]


def test_observer_exception_does_not_propagate(
    caplog: pytest.LogCaptureFixture,
) -> None:
    failing = FailingObserver()
    recording = RecordingObserver()
    event = make_event()
    register_cube_output_observer(failing)
    register_cube_output_observer(recording)
    try:
        with caplog.at_level(logging.ERROR):
            notify_cube_output_observers(event)
    finally:
        unregister_cube_output_observer(failing)
        unregister_cube_output_observer(recording)

    assert recording.events == [event]
    assert "Cube output observer failed" in caplog.text


def load_cube_output_events_module(module_name: Any) -> Any:
    """Load cube output events under a synthetic module identity for regression tests."""

    module_path = (
        Path(__file__).resolve().parents[1]
        / "sugarcubes"
        / "runtime"
        / "cube_output_events.py"
    )
    spec = importlib.util.spec_from_file_location(module_name, module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        sys.modules.pop(module_name, None)
    return module
