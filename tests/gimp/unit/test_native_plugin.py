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
"""Prove native menu registration binds one explicit image and reports launch failures through GIMP."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from types import SimpleNamespace
from typing import cast
from unittest.mock import Mock

import pytest

from integrations.gimp.native.plugin import PROCEDURE, create_plugin
from integrations.gimp.native.plugin_api import (
    NativePlugin,
    PluginApi,
    PluginErrors,
    PluginProcedure,
    RunImage,
)
from tests.gimp.support.native_images import FakeImage


class FakePlugin:
    """Model only the native plug-in process's wire-processing capability."""

    enabled = False

    def persistent_enable(self) -> None:
        """Record native communication admission without starting a GUI."""
        self.enabled = True


@dataclass(frozen=True)
class Outcome:
    """Retain the native procedure result supplied by production registration."""

    status: int
    error: object


def fixture(launch: Callable[[int], None]) -> tuple[FakePlugin, Mock, RunImage]:
    """Replace dynamic GI symbols while retaining the production callback and lifetime owner."""
    procedure = Mock(spec=PluginProcedure)
    procedure.new_return_values.side_effect = Outcome
    runs: list[RunImage] = []

    def new(
        plugin: NativePlugin, name: str, kind: int, run: RunImage, data: object
    ) -> PluginProcedure:
        """Capture the SDK registration boundary for direct native-callback proofs."""
        assert name == PROCEDURE and kind == 1 and data is None
        runs.append(run)
        return cast(PluginProcedure, procedure)

    def failure(domain: int, message: str, code: int) -> str:
        """Model GLib's bounded host-facing error message."""
        assert domain == 7 and code == 1
        return message

    errors = Mock(return_value=None)
    errors.new_literal.side_effect = failure
    # GI's module and GObject class are dynamic external boundaries in both production and this fixture.
    api = cast(
        PluginApi,
        SimpleNamespace(
            PlugIn=FakePlugin,
            ImageProcedure=SimpleNamespace(new=new),
            PDBProcType=SimpleNamespace(PLUGIN=1),
            PDBStatusType=SimpleNamespace(
                SUCCESS=0, CALLING_ERROR=1, EXECUTION_ERROR=2
            ),
            RunMode=SimpleNamespace(INTERACTIVE=0),
        ),
    )
    glib = cast(
        PluginErrors, SimpleNamespace(Error=errors, quark_from_string=lambda name: 7)
    )
    plugin = create_plugin(api, glib, launch)()
    assert isinstance(plugin, FakePlugin)
    query: object = getattr(plugin, "do_query_procedures")
    create: object = getattr(plugin, "do_create_procedure")
    assert callable(query) and callable(create)
    assert query() == [PROCEDURE]
    assert create("unrelated-procedure") is None
    assert create(PROCEDURE) is procedure
    return plugin, procedure, runs[0]


def test_menu_uses_the_public_image_procedure_and_windows_menu() -> None:
    """Declare one native action without changing global image-selection policy."""
    _, procedure, _ = fixture(lambda image_id: None)
    procedure.set_menu_label.assert_called_once_with("SugarCubes…")
    procedure.add_menu_path.assert_called_once_with("<Image>/Windows")
    procedure.set_image_types.assert_called_once_with("*")


def test_interactive_callback_uses_only_the_invoked_image() -> None:
    """Enable modeless communication and pass the exact launch image to the viewer."""
    launched: list[int] = []
    plugin, procedure, run = fixture(launched.append)
    result = run(
        cast(PluginProcedure, procedure), 0, FakeImage(image_id=37), [], None, None
    )
    assert result == Outcome(0, None)
    assert plugin.enabled
    assert launched == [37]


@pytest.mark.parametrize("valid", [True, False])
def test_native_failure_is_not_reported_as_a_success(valid: bool) -> None:
    """Return actionable host errors and retain exception context for diagnostics."""

    def launch(image_id: int) -> None:
        """Fail at the actual external viewer boundary."""
        raise RuntimeError("The packaged companion is unavailable.")

    _, procedure, run = fixture(launch)
    result = run(
        cast(PluginProcedure, procedure), 0, FakeImage(valid=valid), [], None, None
    )
    assert isinstance(result, Outcome)
    assert result.status == 2
    assert (
        "unavailable" in str(result.error) if valid else "closed" in str(result.error)
    )


def test_noninteractive_calls_do_not_open_a_window() -> None:
    """Keep automation from unexpectedly opening the modeless UI."""
    launched: list[int] = []
    plugin, procedure, run = fixture(launched.append)
    result = run(cast(PluginProcedure, procedure), 1, FakeImage(), [], None, None)
    assert isinstance(result, Outcome) and result.status == 1
    assert not plugin.enabled and not launched
