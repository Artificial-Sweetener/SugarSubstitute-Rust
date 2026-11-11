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
"""Own a companion child process and its native SDK resources for one explicit launch image."""

from __future__ import annotations

from collections.abc import Callable
import logging
import os
from pathlib import Path
import subprocess
import sys

from .contract import NativeContract
from .channels import NativeChannels
from .channel_editing import NativeChannelEditing
from .dispatch import NativeDispatch
from .environment import companion_environment
from .files import NativePngFiles
from .images import resolve_image
from .identity import windows_process_identity
from .loop_api import NativeLoopApi
from .outputs import NativeOutputs
from .pixels import NativePixelReader
from .requests import NativeRequests
from .sdk import NativeSdk, host_process_id, host_window_handle
from .snapshots import NativeSnapshots
from .streams import NativePipe
from .viewer_runtime import verify_viewer_runtime

_logger = logging.getLogger(__name__)


def _report(error: Exception) -> None:
    """Preserve native failure context without logging image data or private transfer paths."""
    _logger.error(
        "GIMP companion operation failed",
        exc_info=(type(error), error, error.__traceback__),
    )


def run_native_viewer(
    image_id: int, executable: Path, sdk: NativeSdk, glib: NativeLoopApi
) -> None:
    """Run the packaged companion while retaining all GI calls on the plug-in's main thread.

    Args:
        image_id: Explicit image supplied by the image procedure invocation.
        executable: Trusted companion executable resolved inside the installed extension package.
        sdk: Installed GIMP collaborators.
        glib: Installed GLib event-loop services.

    Raises:
        RuntimeError: The package, viewer process or native cleanup failed.
        ExceptionGroup: More than one shutdown collaborator failed.
    """
    image = resolve_image(sdk.gimp.Image, image_id)
    if not executable.is_file() or executable.suffix.lower() != ".exe":
        raise RuntimeError("The installed GIMP companion executable is unavailable.")
    environment = companion_environment(os.environ)
    environment["SUGARCUBES_GIMP_IMAGE_NAME"] = image.get_name() or "Untitled image"
    parent_handle = host_window_handle()
    if parent_handle is not None:
        environment["SUGARCUBES_GIMP_OWNER"] = str(parent_handle)
    verify_viewer_runtime(executable, environment)
    contract = NativeContract()
    files = NativePngFiles(sdk.gio, contract.limits.png_bytes, _report)
    dispatch = NativeDispatch(
        sdk,
        image_id,
        NativeSnapshots(
            sdk.gimp, NativePixelReader(sdk.gegl, contract.limits.raster_bytes)
        ),
        NativeOutputs(sdk.output, files),
        contract.limits.png_bytes,
        windows_process_identity(host_process_id()),
        NativeChannels(sdk.channels, sdk.gegl, contract.limits.raster_bytes, _report),
        NativeChannelEditing(sdk.channels, _report),
    )
    loop = glib.MainLoop.new(None, False)
    child = subprocess.Popen(
        [str(executable)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr,
        env=environment,
        creationflags=subprocess.CREATE_NO_WINDOW,
    )
    pipe: NativePipe | None = None
    try:
        if child.stdin is None or child.stdout is None:
            raise RuntimeError("The companion did not expose its private binary pipes.")

        def schedule(callback: Callable[[], None]) -> None:
            """Marshal pipe-worker requests onto GLib without allowing nested SDK dispatch."""

            def invoke() -> bool:
                """Run one callback and retire this idle source."""
                callback()
                return False

            glib.idle_add(invoke)

        pipe = NativePipe(
            child.stdout,
            child.stdin,
            contract.limits.frame_bytes,
            lambda frame: requests.receive(frame),
            lambda: requests.close(),
            _report,
            contract.limits.pending_requests,
        )
        requests = NativeRequests(
            contract, dispatch, schedule, pipe.send, _report, loop.quit
        )
        pipe.start()
        loop.run()
    finally:
        errors: list[Exception] = []
        if pipe is not None:
            pipe.stop()
        try:
            _stop_child(child)
        except Exception as error:
            errors.append(error)
        if pipe is not None:
            try:
                pipe.join()
            except Exception as error:
                errors.append(error)
        for stream in (child.stdin, child.stdout):
            if stream is not None:
                try:
                    stream.close()
                except OSError as error:
                    errors.append(error)
        for owner in (dispatch, files):
            try:
                owner.close()
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("GIMP companion shutdown failed.", errors)
    if child.returncode != 0:
        raise RuntimeError(f"The GIMP companion exited with status {child.returncode}.")


def _stop_child(child: subprocess.Popen[bytes]) -> None:
    """Stop only the child created by this plug-in and wait before closing shared pipe handles."""
    if child.poll() is not None:
        return
    child.terminate()
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait(timeout=5)
