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
"""Qualify staged native capture modules inside an explicitly isolated GIMP instance."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass
import json
from pathlib import Path
import traceback
from typing import TypedDict

from integrations.gimp import native
from integrations.gimp.native.image_api import NativeImage
from integrations.gimp.native.images import describe_image
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.sdk import load_sdk
from integrations.gimp.native.snapshots import NativeSnapshots, NativeVisibility


@dataclass(frozen=True, slots=True)
class NativeProjectionCheck:
    """Provide expected fixture pixels without importing the dynamic SDK into proof logic."""

    name: str
    visibility: tuple[NativeVisibility, ...]
    expected: bytes


class NativeProofReport(TypedDict):
    """Distinguish native pixel qualification from interactive GUI lifecycle testing."""

    passed: bool
    checks: list[str]
    module: str
    failure: str | None
    gui_lifecycle_tested: bool


def prove_staged_snapshots(
    image: NativeImage,
    cases: Sequence[NativeProjectionCheck],
    selection: bytes,
    edit_original: Callable[[], None],
    fresh_projection: bytes,
    staged_root: Path,
    output: Path,
    expected_primary: int | None,
) -> None:
    """Read actual native pixels and prove cleanup while keeping fixture creation in the host harness."""
    sdk = load_sdk()
    document = describe_image(image)
    report: NativeProofReport = {
        "passed": False,
        "checks": [],
        "module": native.__file__ or "",
        "failure": None,
        "gui_lifecycle_tested": False,
    }
    snapshots = NativeSnapshots(
        sdk.gimp, NativePixelReader(sdk.gegl, document.width * document.height * 4)
    )
    before = {item.get_id() for item in sdk.gimp.get_images()}
    try:
        try:
            if not report["module"] or not Path(
                report["module"]
            ).resolve().is_relative_to(staged_root.resolve()):
                raise AssertionError(
                    "The native proof did not import the staged plug-in package."
                )
            report["checks"].append("staged_package_import")
            info = snapshots.begin(document.image_id, document.revision)
            if info.primary_layer != expected_primary:
                raise AssertionError(
                    "The frozen primary layer differs from the native launch selection."
                )
            report["checks"].append("frozen_primary_layer")
            if len(sdk.gimp.get_images()) != len(before) + 1:
                raise AssertionError(
                    "The native batch did not retain exactly one offscreen image."
                )
            report["checks"].append("one_offscreen_snapshot")
            edit_original()
            for case in cases:
                result = snapshots.projection(
                    info.token, case.visibility, document.width, document.height, 1
                )
                if result.data != case.expected:
                    raise AssertionError(f"Native pixels disagree for {case.name}.")
                report["checks"].append(case.name)
            coverage = snapshots.selection(
                info.token, document.width, document.height, 1
            )
            if coverage.data != selection:
                raise AssertionError(
                    "Native frozen selection coverage differs from the fixture."
                )
            report["checks"].append("frozen_selection_pixels")
            snapshots.release(info.token)
            fresh = snapshots.begin(document.image_id, document.revision)
            if (
                snapshots.projection(
                    fresh.token, (), document.width, document.height, 1
                ).data
                != fresh_projection
            ):
                raise AssertionError(
                    "A new batch did not capture fresh native contents."
                )
            report["checks"].append("fresh_execution_pixels")
        finally:
            snapshots.close()
        if {
            item.get_id() for item in sdk.gimp.get_images()
        } != before or not image.is_valid():
            raise AssertionError(
                "Native snapshot cleanup changed source images or leaked temporary images."
            )
        report["checks"].append("native_image_cleanup")
        report["passed"] = True
    except Exception:
        report["failure"] = traceback.format_exc()
        raise
    finally:
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")
