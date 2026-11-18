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
"""Run independent validation processes side by side within one bounded budget."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import threading
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

JOBS_VARIABLE = "SUGARCUBES_GATE_JOBS"
"""Environment override for how many budget units run at once."""

DEFAULT_JOBS = 8
"""Units that run at once unless overridden: enough to overlap every stage of the gate while a
developer machine keeps the memory its other work needs."""


@dataclass(frozen=True, slots=True)
class Command:
    """Name one process, where it runs, and how much of the budget it occupies.

    A process that fans out its own workers states their number as its weight, so the budget
    bounds what actually runs at once rather than how many processes were started.
    """

    label: str
    cwd: Path
    arguments: tuple[str, ...]
    weight: int = 1


def gate_jobs(environment: Mapping[str, str] | None = None) -> int:
    """Read the budget, refusing an override that is not a positive whole number."""

    value = (os.environ if environment is None else environment).get(JOBS_VARIABLE)
    if value is None or not value.strip():
        return DEFAULT_JOBS
    try:
        jobs = int(value)
    except ValueError as error:
        raise ValueError(f"{JOBS_VARIABLE} must be a positive whole number.") from error
    if jobs < 1:
        raise ValueError(f"{JOBS_VARIABLE} must be a positive whole number.")
    return jobs


def run_commands(
    commands: Sequence[Command],
    *,
    jobs: int,
    environment: Mapping[str, str] | None = None,
) -> int:
    """Run every command within the budget and return the first failing status in given order.

    Each process writes to its own file and its output is printed whole when it ends, so
    concurrent processes never interleave. After a failure nothing new starts; processes already
    running finish and report, so one run surfaces every failure it reached.
    """

    if jobs < 1:
        raise ValueError("The parallel budget must be at least one unit.")
    if not commands:
        return 0
    statuses: dict[int, int] = {}
    state = threading.Condition()
    available = jobs
    failed = False

    def work(index: int, command: Command, weight: int) -> None:
        nonlocal available, failed
        started = time.monotonic()
        with tempfile.TemporaryFile() as output:
            try:
                status = subprocess.run(  # noqa: S603
                    command.arguments,
                    cwd=command.cwd,
                    check=False,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                ).returncode
            except OSError as error:
                output.write(f"{error}\n".encode())
                status = 127
            output.seek(0)
            text = output.read()
        seconds = time.monotonic() - started
        with state:
            verdict = "passed" if status == 0 else f"FAILED with status {status}"
            sys.stdout.flush()
            sys.stdout.buffer.write(
                f"\n===== {command.label}: {verdict} in {seconds:.0f} s =====\n".encode()
            )
            sys.stdout.buffer.write(text)
            if text and not text.endswith(b"\n"):
                sys.stdout.buffer.write(b"\n")
            sys.stdout.buffer.flush()
            statuses[index] = status
            failed = failed or status != 0
            available += weight
            state.notify_all()

    threads: list[threading.Thread] = []
    for index, command in enumerate(commands):
        # A command heavier than the whole budget runs alone rather than never.
        weight = max(1, min(command.weight, jobs))
        with state:
            while not failed and available < weight:
                state.wait()
            if failed:
                break
            available -= weight
        thread = threading.Thread(
            target=work, args=(index, command, weight), name=command.label
        )
        thread.start()
        threads.append(thread)
    for thread in threads:
        thread.join()
    return next(
        (statuses[index] for index in sorted(statuses) if statuses[index] != 0), 0
    )


__all__ = ["Command", "DEFAULT_JOBS", "JOBS_VARIABLE", "gate_jobs", "run_commands"]
