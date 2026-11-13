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
"""Qualify staged native output insertion inside an isolated installed GIMP runtime."""

from __future__ import annotations

from contextlib import ExitStack
import json
from pathlib import Path
import traceback

from integrations.gimp.native.files import NativePngFiles
from integrations.gimp.native.image_api import NativeImage
from integrations.gimp.native.images import describe_image
from integrations.gimp.native.layers import layer_catalog
from integrations.gimp.native.model import NativeOperationCancelled, NativePng
from integrations.gimp.native.outputs import NativeOutputs
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.sdk import load_sdk
from integrations.gimp.native.snapshots import NativeSnapshots


def prove_native_output(
    image: NativeImage,
    png: NativePng,
    left: int,
    top: int,
    expected_frame: bytes,
    expected_selection: bytes,
    output: Path,
) -> None:
    """Compare native destination pixels after decoding, cancellation and one committed insertion."""
    sdk = load_sdk()
    document = describe_image(image)
    checks: list[str] = []
    errors: list[Exception] = []
    report: dict[str, object] = {
        "passed": False,
        "checks": checks,
        "gui_lifecycle_tested": False,
    }
    before = {item.get_id() for item in sdk.gimp.get_images()}
    catalog = layer_catalog(image)
    selected = tuple(layer.get_tattoo() for layer in image.get_selected_layers())
    try:
        with ExitStack() as resources:
            files = NativePngFiles(sdk.gio, len(png.data), errors.append)
            resources.callback(files.close)
            outputs = NativeOutputs(sdk.output, files)
            resources.callback(outputs.close)
            snapshots = NativeSnapshots(
                sdk.gimp,
                NativePixelReader(sdk.gegl, document.width * document.height * 4),
            )
            resources.callback(snapshots.close)
            if image.is_dirty():
                raise AssertionError(
                    "The output fixture must start as a clean saved image."
                )
            calls = 0

            def cancel_after_preparation() -> bool:
                """Allow decoding and off-image preparation before cancelling the mutation."""
                nonlocal calls
                calls += 1
                return calls > 1

            try:
                outputs.insert(
                    document,
                    png,
                    "Cancelled output",
                    left,
                    top,
                    cancel_after_preparation,
                )
            except NativeOperationCancelled:
                pass
            else:
                raise AssertionError(
                    "Native insertion did not honor precommit cancellation."
                )
            if (
                image.is_dirty()
                or layer_catalog(image) != catalog
                or tuple(layer.get_tattoo() for layer in image.get_selected_layers())
                != selected
            ):
                raise AssertionError(
                    "Cancelled native preparation modified the source image."
                )
            checks.append("cancelled_preparation_preserves_clean_document")
            result = outputs.insert(
                document, png, "<literal output>", left, top, lambda: False
            )
            if (result.left, result.top, result.width, result.height) != (
                left,
                top,
                png.width,
                png.height,
            ):
                raise AssertionError(
                    "Native output altered the shared placement or intrinsic dimensions."
                )
            layer = image.get_layer_by_tattoo(result.tattoo)
            if (
                layer is None
                or layer.get_name() != "<literal output>"
                or not image.is_dirty()
            ):
                raise AssertionError(
                    "Native output did not commit the prepared layer identity and label."
                )
            if (
                len(layer_catalog(image)) != len(catalog) + 1
                or describe_image(image) != document
            ):
                raise AssertionError(
                    "Native output changed the target geometry or inserted multiple layers."
                )
            checks.append("single_native_layer_commit")
            batch = snapshots.begin(document.image_id, document.revision)
            if (
                snapshots.projection(
                    batch.token, (), document.width, document.height, 1
                ).data
                != expected_frame
            ):
                raise AssertionError(
                    "Native output pixels do not match the expected placement."
                )
            checks.append("exact_native_placement_with_active_selection")
            if (
                snapshots.selection(
                    batch.token, document.width, document.height, 1
                ).data
                != expected_selection
            ):
                raise AssertionError("Native output changed the existing selection.")
            checks.append("native_selection_preserved")
            report["layer_tattoo"] = result.tattoo
        if (
            {item.get_id() for item in sdk.gimp.get_images()} != before
            or not layer.is_valid()
            or errors
        ):
            raise AssertionError(
                "Native output cleanup leaked a resource or deleted the committed layer."
            )
        checks.append("native_output_cleanup")
        report["passed"] = True
    except Exception:
        report["failure"] = traceback.format_exc()
        raise
    finally:
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")
