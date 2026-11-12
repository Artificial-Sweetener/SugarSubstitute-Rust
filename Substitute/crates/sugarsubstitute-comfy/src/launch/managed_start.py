# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Run ComfyUI with a shutdown channel owned by the desktop process family."""
import _thread
import os
import runpy
import sys
import threading

sys.argv = sys.argv[1:]
sys.path.insert(0, os.path.dirname(sys.argv[0]))
finished = threading.Event()


def prepare_custom_nodes():
    """Keep installed nodes discoverable when mutable Comfy paths use another base."""
    import comfy.options
    comfy.options.enable_args_parsing()
    import folder_paths
    folder_paths.add_model_folder_path(
        "custom_nodes", os.path.join(os.path.dirname(sys.argv[0]), "custom_nodes")
    )


def prepare_shutdown_input():
    """Keep the shutdown pipe private; backend libraries receive ordinary null stdin."""
    descriptor = os.dup(sys.stdin.fileno())
    with open(os.devnull, "rb", buffering=0) as null_input:
        os.dup2(null_input.fileno(), sys.stdin.fileno())
    if os.name == "nt":
        import ctypes
        from ctypes import wintypes
        import msvcrt

        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.SetStdHandle.argtypes = [wintypes.DWORD, wintypes.HANDLE]
        kernel.SetStdHandle.restype = wintypes.BOOL
        if not kernel.SetStdHandle(-10, msvcrt.get_osfhandle(sys.stdin.fileno())):
            os.close(descriptor)
            raise ctypes.WinError(ctypes.get_last_error())
    return descriptor


shutdown_input = prepare_shutdown_input()


def wait_for_shutdown():
    """A private descriptor cannot block libraries inspecting or configuring stdin."""
    try:
        while os.read(shutdown_input, 1):
            pass
    finally:
        os.close(shutdown_input)

def observe_desktop():
    """EOF requests Python's normal interrupt handler, including ComfyUI cleanup."""
    try:
        wait_for_shutdown()
    finally:
        if not finished.is_set():
            _thread.interrupt_main()


# This single observer belongs to this backend process. Its only blocking call
# ends on desktop shutdown; daemon teardown also permits ComfyUI to exit first.
threading.Thread(target=observe_desktop, name="SugarSubstitute shutdown", daemon=True).start()
try:
    prepare_custom_nodes()
