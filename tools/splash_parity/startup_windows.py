# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Measure Rust and the production PySide splash through the same external Windows observer."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

try:
    from .native_windows import NativePointer, NativeWindows, observe_compositor, rgb_pixel, write_json
    from .native_decisions import startup_decision
except ImportError:
    from native_windows import NativePointer, NativeWindows, observe_compositor, rgb_pixel, write_json
    from native_decisions import startup_decision


def launch(arguments: argparse.Namespace, output: Path) -> tuple[list[str], Path, dict[str, str]]:
    """Use actual production entrypoints, preserving interpreter and font/cache state."""
    environment = os.environ.copy()
    temporary = arguments.temporary.resolve(strict=True)
    environment.update(TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1")
    # A preceding offscreen capture must not turn native qualification into a hidden Qt run.
    environment.pop("QT_QPA_PLATFORM", None)
    if arguments.mode == "reference":
        root = arguments.legacy_root.resolve(strict=True)
        interpreter = arguments.interpreter.resolve(strict=True)
        command = [str(interpreter), "-m", "substitute.app.bootstrap.shared_splash_host",
                   "--maximum-lifetime-seconds", str(arguments.duration_ms / 1000),
                   "--theme-mode", "light" if arguments.light else "dark",
                   "--backdrop-mode", "mica", "--locale", arguments.locale]
        return command, root, environment
    executable = arguments.executable.resolve(strict=True)
    command = [str(executable), "--duration-ms", str(arguments.duration_ms), "--collapsed",
               "--receipt", str(output / "submission.json"), "--locale", arguments.locale]
    if arguments.light:
        command.append("--light")
    return command, executable.parent, environment


def observe(arguments: argparse.Namespace, output: Path, ordinal: int,
            native: NativeWindows) -> dict[str, object]:
    """Observe compositor paint and a causal caption response before writing bulky images."""
    output.mkdir(parents=True, exist_ok=False)
    command, cwd, environment = launch(arguments, output)
    receipt: dict[str, object] = {
        "mode": arguments.mode, "run_ordinal": ordinal,
        "run_class": "first_in_series" if ordinal == 0 else "warm_repeat",
        "cache_policy": "Existing font and application caches retained; no cold-cache claim",
        "command": command, "cwd": str(cwd),
        "entrypoint_sha256": hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),
    }
    if arguments.mode == "reference":
        receipt["reference_source_sha256"] = {
            name: hashlib.sha256((cwd / name).read_bytes()).hexdigest()
            for name in (
                "substitute/app/bootstrap/shared_splash_host.py",
                "substitute/presentation/shell/splash_window.py",
                "substitute/presentation/shell/splash_feedback.py",
                "substitute/presentation/shell/splash_progress_panel.py",
            )
        }
    started = time.perf_counter()
    process = subprocess.Popen(command, cwd=cwd, env=environment,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    receipt["pid"] = process.pid
    try:
        deadline = started + arguments.duration_ms / 1000
        hwnd = native.find_window(process, deadline)
        client = native.client(hwnd, process.pid)
        receipt.update(dpi=client.dpi, device_size=[client.width, client.height],
                       logical_size=[client.width / client.scale, client.height / client.scale])
        image, observed = observe_compositor(native, hwnd, process, deadline)
        visible_ms = (observed - started) * 1000
        before = rgb_pixel(image, (round(30 * client.scale), round(400 * client.scale)))
        # Both public surfaces begin collapsed. One real caption press/release expands output.
        receipt["caption_gesture"] = {"logical_point": [489, 16],
                                      "device_point": [round(489 * client.scale), round(16 * client.scale)],
                                      "input_source": "SendInput native mouse down/move/up"}
        with NativePointer(native, hwnd, process.pid) as pointer:
            pointer.move((489, 16))
            pointer.press()
            pointer.release()
            while True:
                changed = native.capture(hwnd, process.pid, compositor=True)
                after = rgb_pixel(changed, (round(30 * client.scale), round(400 * client.scale)))
                if max(abs(left - right) for left, right in zip(before, after)) > 5:
                    break
                if time.perf_counter() >= deadline or process.poll() is not None:
                    raise RuntimeError("The caption disclosure did not visibly respond to its first gesture")
                time.sleep(0.005)
            interactive_ms = (time.perf_counter() - started) * 1000
        receipt.update(startup=startup_decision(visible_ms, interactive_ms),
                       observation_method="Owned unoccluded DWM compositor capture; first caption gesture response")
        image.save(output / "visible.png")
        changed.save(output / "interactive.png")
        native.post(hwnd, process.pid, 0x0010, 0, 0)
        stdout, stderr = process.communicate(timeout=5)
        # The production reference's ready message contains session authentication material.
        receipt.update(exit_code=process.returncode, stdout_bytes=len(stdout), stderr_bytes=len(stderr))
        if process.returncode != 0:
            raise RuntimeError(f"Owned startup fixture exited with status {process.returncode}")
        write_json(output / "startup.json", receipt)
        return receipt
    except Exception as error:
        receipt.update(error_type=type(error).__name__, error=str(error))
        write_json(output / "failure.json", receipt)
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.communicate(timeout=5)


def main() -> int:
    """Run sequential windows and keep first-in-series measurements separate from warm repeats."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("rust", "reference"))
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--interpreter", type=Path)
    parser.add_argument("--legacy-root", type=Path)
    parser.add_argument("--temporary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--locale", default="en", choices=("en", "zh-Hans", "ja", "ko", "es"))
    parser.add_argument("--light", action="store_true")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--duration-ms", type=int, default=5000)
    parser.add_argument("--enforce-targets", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.runs <= 30 or not 1000 <= args.duration_ms <= 30000:
        parser.error("Use 1 through 30 sequential runs with a 1000 through 30000 ms lifetime")
    if args.mode == "rust" and args.executable is None:
        parser.error("Rust qualification requires --executable")
    if args.mode == "reference" and (args.interpreter is None or args.legacy_root is None):
        parser.error("Reference qualification requires --interpreter and --legacy-root")
    args.output.mkdir(parents=True, exist_ok=False)
    native = NativeWindows()
    receipts = [observe(args, args.output / f"run-{ordinal:02}", ordinal, native) for ordinal in range(args.runs)]
    write_json(args.output / "series.json", receipts)
    return 1 if args.enforce_targets and any(not receipt["startup"]["accepted"] for receipt in receipts) else 0


if __name__ == "__main__":
    raise SystemExit(main())
