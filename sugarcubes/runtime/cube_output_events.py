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
"""Publish neutral SugarCubes output events to process-local observers."""

from __future__ import annotations

import logging
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from types import ModuleType
from typing import Literal, Protocol, cast

LOGGER = logging.getLogger(__name__)
CUBE_OUTPUT_OBSERVER_API_VERSION = 1
_EVENT_BUS_MODULE_NAME = "_sugarcubes_cube_output_event_bus_v1"

MediaKind = Literal["image", "audio", "video", "value", "unknown"]


@dataclass(frozen=True)
class CubeOutputArtifact:
    """Describe one Comfy-visible artifact produced for a cube output."""

    filename: str
    subfolder: str
    type: str
    media_kind: MediaKind
    mime_type: str | None = None
    width: int | None = None
    height: int | None = None
    duration_seconds: float | None = None


@dataclass(frozen=True)
class CubeOutputEvent:
    """Describe one executed SugarCubes output boundary."""

    version: int
    prompt_id: str | None
    node_id: str | None
    list_index: int | None
    cube_id: str
    default_alias: str
    instance_alias: str
    instance_id: str
    media_kind: MediaKind
    value_type: str
    artifacts: tuple[CubeOutputArtifact, ...]


class CubeOutputObserver(Protocol):
    """Observe executed cube outputs without owning SugarCubes behavior."""

    def on_cube_output(self, event: CubeOutputEvent) -> None:
        """Handle one cube output event."""


class CubeOutputEventBus:
    """Own process-local cube-output observer delivery independent of import name."""

    def __init__(self) -> None:
        """Initialize an empty observer registry."""

        self._observers: list[CubeOutputObserver] = []

    def register(self, observer: CubeOutputObserver) -> None:
        """Register one observer without duplicating delivery."""

        if observer in self._observers:
            return
        self._observers.append(observer)

    def unregister(self, observer: CubeOutputObserver) -> None:
        """Unregister one observer, tolerating stale unregister requests."""

        try:
            self._observers.remove(observer)
        except ValueError:
            LOGGER.debug("Ignored unregister request for unknown cube output observer.")

    def notify(self, event: CubeOutputEvent) -> None:
        """Deliver one event without letting observer failures break generation."""

        for observer in self.snapshot():
            try:
                observer.on_cube_output(event)
            except Exception:
                LOGGER.exception(
                    "Cube output observer failed; generation will continue.",
                    extra={
                        "prompt_id": event.prompt_id,
                        "node_id": event.node_id,
                        "cube_id": event.cube_id,
                        "default_alias": event.default_alias,
                        "instance_id": event.instance_id,
                    },
                )

    def snapshot(self) -> Sequence[CubeOutputObserver]:
        """Return a stable observer delivery snapshot."""

        return tuple(self._observers)


def register_cube_output_observer(observer: CubeOutputObserver) -> None:
    """Register one process-local cube output observer."""

    _event_bus().register(observer)


def unregister_cube_output_observer(observer: CubeOutputObserver) -> None:
    """Unregister one process-local cube output observer."""

    _event_bus().unregister(observer)


def notify_cube_output_observers(event: CubeOutputEvent) -> None:
    """Deliver one cube output event without letting observers break generation."""

    _event_bus().notify(event)


def _snapshot_observers() -> Sequence[CubeOutputObserver]:
    """Return a stable observer delivery snapshot."""

    return _event_bus().snapshot()


def _event_bus() -> CubeOutputEventBus:
    """Return the canonical event bus shared across SugarCubes import identities."""

    module = sys.modules.get(_EVENT_BUS_MODULE_NAME)
    if module is None:
        module = ModuleType(_EVENT_BUS_MODULE_NAME)
        sys.modules[_EVENT_BUS_MODULE_NAME] = module
    bus = getattr(module, "event_bus", None)
    if bus is None:
        bus = CubeOutputEventBus()
        setattr(module, "event_bus", bus)
    return cast(CubeOutputEventBus, bus)
