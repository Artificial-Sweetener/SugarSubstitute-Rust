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
"""Prove immutable queued policies and atomic numbering with a real SQLite journal."""

from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from datetime import datetime
from pathlib import Path
import sqlite3

import pytest

from sugarcubes.output_saving.domain.preferences import OutputSavingPreferences
from sugarcubes.output_saving.domain.run_plan import SavingRun, SavingSource, SourceKind
from sugarcubes.output_saving.infrastructure.ticket_journal import SavingTicketJournal


def _run(identity: str) -> SavingRun:
    """Supply exact seed and workflow metadata values without a mounted browser."""
    return SavingRun(
        identity,
        "E:/Outputs",
        "Example",
        datetime(2026, 5, 1),
        OutputSavingPreferences(),
        (("workflow", '{"nodes":[]}'),),
    )


def test_ticket_survives_reopening_without_inheriting_changed_settings(
    tmp_path: Path,
) -> None:
    """Read the original frozen policy and exact seed from a new journal instance."""
    path = tmp_path / "saving.sqlite"
    journal = SavingTicketJournal(path)
    run = _run("run-a")
    source = SavingSource(
        "sink",
        "Final",
        SourceKind.CUBE,
        cube_id="a",
        seed="18319146648460766796",
        persist=True,
    )
    ticket = journal.issue_run(run, (source,))[0]
    altered = replace(
        run, run_id="run-b", preferences=replace(run.preferences, manage_workflow=True)
    )
    journal.issue_run(altered, ())
    restored = SavingTicketJournal(path).read_ticket(ticket)
    assert restored.run == run
    assert restored.source == source
    assert restored.source.seed == "18319146648460766796"
    assert not restored.run.preferences.manage_workflow
    assert journal.next_source_index(ticket) == 1
    assert SavingTicketJournal(path).next_source_index(ticket) == 2


def test_unknown_ticket_and_reused_run_are_rejected(tmp_path: Path) -> None:
    """Never grant output path authority to arbitrary client-provided tokens."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    with pytest.raises(ValueError):
        journal.read_ticket("unknown")
    with pytest.raises(ValueError):
        journal.next_source_index("unknown")
    run = _run("run-a")
    journal.issue_run(run, ())
    with pytest.raises(sqlite3.IntegrityError):
        journal.issue_run(replace(run, root="E:/Different"), ())


def test_run_number_is_shared_across_sources_and_scans_once_per_bucket(
    tmp_path: Path,
) -> None:
    """Use one committed namespace counter instead of allocating a number for each cube."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    scans: list[str] = []

    def first_available() -> int:
        """Represent the filesystem scan behind the explicit counter bootstrap boundary."""
        scans.append("scan")
        return 8

    journal.issue_run(_run("run-a"), ())
    journal.issue_run(_run("run-b"), ())
    assert journal.run_number("run-a", "folder", first_available) == 8
    assert journal.run_number("run-a", "folder", first_available) == 8
    assert journal.run_number("run-b", "folder", first_available) == 9
    assert scans == ["scan"]


def test_independent_writers_allocate_unique_folder_ordinals(tmp_path: Path) -> None:
    """Exercise cross-instance SQLite serialization instead of a process-local numbering cache."""
    path = tmp_path / "saving.sqlite"
    SavingTicketJournal(path)

    def reserve(index: int) -> int:
        """Use independent journal connections racing for the same folder namespace."""
        return SavingTicketJournal(path).image_number("folder", lambda: 1)

    with ThreadPoolExecutor(max_workers=4) as executor:
        numbers = list(executor.map(reserve, range(12)))
    assert sorted(numbers) == list(range(1, 13))


def test_incomplete_ticket_issue_rolls_back_the_whole_generation(
    tmp_path: Path,
) -> None:
    """Leave no partial queued policy when a later source violates an identity constraint."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    source = SavingSource("sink", "Result", SourceKind.CUBE)
    run = _run("run-a")
    with pytest.raises(sqlite3.IntegrityError):
        journal.issue_run(run, (source, source))
    ticket = journal.issue_run(run, (source,))[0]
    assert journal.read_ticket(ticket).run == run


def test_future_journal_version_is_not_modified(tmp_path: Path) -> None:
    """Reject unsupported storage before creating or replacing any future-version tables."""
    path = tmp_path / "saving.sqlite"
    with sqlite3.connect(path) as connection:
        connection.execute("CREATE TABLE journal_schema(version INTEGER NOT NULL)")
        connection.execute("INSERT INTO journal_schema VALUES (2)")
    previous = path.read_bytes()
    with pytest.raises(ValueError):
        SavingTicketJournal(path)
    assert path.read_bytes() == previous


def test_terminal_run_cleanup_releases_metadata_without_resetting_shared_counters(
    tmp_path: Path,
) -> None:
    """Retire only the observed terminal generation and preserve another queued policy."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    source = SavingSource("sink", "Result", SourceKind.CUBE)
    first = journal.issue_run(_run("run-a"), (source,))[0]
    second = journal.issue_run(_run("run-b"), (source,))[0]
    assert journal.run_number("run-a", "folder", lambda: 8) == 8
    journal.discard_run("run-a")
    with pytest.raises(ValueError):
        journal.read_ticket(first)
    assert journal.read_ticket(second).run.run_id == "run-b"
    assert journal.run_number("run-b", "folder", lambda: 1) == 9
    journal.discard_run("run-a")


def test_restart_retains_only_authoritatively_live_queue_snapshots(
    tmp_path: Path,
) -> None:
    """Release abandoned metadata after a crash while retaining any recovered queued generation."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    source = SavingSource("sink", "Result", SourceKind.CUBE)
    abandoned = journal.issue_run(_run("abandoned"), (source,))[0]
    live = journal.issue_run(_run("live"), (source,))[0]
    journal.retain_runs(frozenset({"live"}))
    with pytest.raises(ValueError):
        journal.read_ticket(abandoned)
    assert journal.read_ticket(live).run.run_id == "live"
