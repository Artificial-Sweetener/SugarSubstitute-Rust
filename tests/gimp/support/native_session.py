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
"""Compose real private-protocol owners with typed external GIMP SDK fixtures."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import tempfile

import pytest

from integrations.gimp.native.contract import NativeContract
from integrations.gimp.native.channels import NativeChannels
from integrations.gimp.native.channel_editing import NativeChannelEditing
from integrations.gimp.native.dispatch import NativeDispatch
from integrations.gimp.native.files import NativePngFiles
from integrations.gimp.native.outputs import NativeOutputs
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.sdk import NativeSdk
from integrations.gimp.native.snapshots import NativeSnapshots
from .native_images import (
    FakeBuffer,
    FakeGeglApi,
    FakeGimpApi,
    FakeImageLookup,
    FakeLayer,
    FakeProjectionFactory,
)
from .native_outputs import FakeGioApi, FakeOutputApi, FakeOutputImage, FakeOutputLookup
from .native_channels import FakeChannelApi, FakeChannelLookup


@dataclass
class NativeSessionFixture:
    """Expose native state for behavioral assertions without mocking protocol dispatch."""

    image: FakeOutputImage
    output: FakeOutputApi
    contract: NativeContract
    dispatch: NativeDispatch
    files: NativePngFiles
    errors: list[Exception]

    def close(self) -> None:
        """Release native and filesystem owners after a proof."""
        try:
            self.dispatch.close()
        finally:
            self.files.close()

    def call(self, request_id: int, command: dict[str, object]) -> dict[str, object]:
        """Apply actual inbound and outbound wire admission around native operations."""
        request = {"type": "request", "id": request_id, "command": command}
        self.contract.decode(self.contract.encode(request))
        result = self.dispatch.execute(request_id, command, lambda: False)
        reply = {"type": "reply", "id": request_id, "result": result}
        self.contract.decode(self.contract.encode(reply))
        return result


def native_session(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> NativeSessionFixture:
    """Inject only SDK/file boundaries while retaining production capture and output ownership."""
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    layer = FakeLayer(FakeBuffer(b""), tattoo=31, name="<literal native layer>")
    image = FakeOutputImage(layers=[layer], selected=[layer])
    sdk = NativeSdk(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), FakeProjectionFactory()),
        FakeGeglApi(),
        FakeOutputApi(FakeOutputLookup(image)),
        FakeGioApi(),
        FakeChannelApi(FakeChannelLookup(image)),
    )
    contract = NativeContract()
    errors: list[Exception] = []
    files = NativePngFiles(sdk.gio, contract.limits.png_bytes, errors.append)
    snapshots = NativeSnapshots(
        sdk.gimp, NativePixelReader(sdk.gegl, contract.limits.raster_bytes)
    )
    output = sdk.output
    if not isinstance(output, FakeOutputApi):
        raise TypeError("The fixture requires its typed native decoder.")
    return NativeSessionFixture(
        image,
        output,
        contract,
        NativeDispatch(
            sdk,
            image.image_id,
            snapshots,
            NativeOutputs(sdk.output, files),
            contract.limits.png_bytes,
            "fixture-host",
            NativeChannels(
                sdk.channels, sdk.gegl, contract.limits.raster_bytes, errors.append
            ),
            NativeChannelEditing(sdk.channels, errors.append),
        ),
        files,
        errors,
    )
