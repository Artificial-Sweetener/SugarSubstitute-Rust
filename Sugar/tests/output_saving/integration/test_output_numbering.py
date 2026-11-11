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
"""Prove real folder numbering respects prior Substitute files and immutable run identities."""

from datetime import datetime
from pathlib import Path

from sugarcubes.output_saving.domain.preferences import (
    OutputOrganization,
    OutputSavingPreferences,
)
from sugarcubes.output_saving.domain.run_plan import SavingRun, SavingSource, SourceKind
from sugarcubes.output_saving.infrastructure.naming_numbers import OutputNamingNumbers
from sugarcubes.output_saving.infrastructure.ticket_journal import SavingTicketJournal


def _issue(
    journal: SavingTicketJournal,
    root: Path,
    identity: str,
    *,
    pattern: str = r"{date}\{run}_{cube#}_{workflow}_{source}",
    persist: bool = True,
) -> str:
    """Create one source under a deterministic run policy for filesystem-backed proof."""
    run = SavingRun(
        identity,
        str(root),
        "Example",
        datetime(2026, 5, 1),
        OutputSavingPreferences(organization=OutputOrganization(path_pattern=pattern)),
    )
    return journal.issue_run(
        run,
        (
            SavingSource(
                "sink",
                "Result",
                SourceKind.CUBE,
                cube_id="a",
                cube_number=1,
                persist=persist,
            ),
        ),
    )[0]


def test_existing_substitute_runs_seed_the_shared_counter_once(tmp_path: Path) -> None:
    """Continue existing default-pattern numbering without rescanning every output image."""
    root = tmp_path / "outputs"
    day = root / "2026-05-01"
    day.mkdir(parents=True)
    (day / "007_01_example_result.png").write_bytes(b"fixture")
    journal = SavingTicketJournal(tmp_path / "state" / "saving.sqlite")
    ticket = _issue(journal, root, "run-a")
    numbers = OutputNamingNumbers(journal)
    first = numbers.context(journal.read_ticket(ticket), width=100, height=80)
    second = numbers.context(journal.read_ticket(ticket), width=100, height=80)
    assert first.run == second.run == 8
    assert first.index == 1
    assert second.index == 2
    assert not (day / "008_01_example_result.png").exists()
    newer = _issue(journal, root, "run-b")
    assert numbers.context(journal.read_ticket(newer), width=100, height=80).run == 9


def test_source_mute_does_not_create_an_output_directory_or_committed_run_number(
    tmp_path: Path,
) -> None:
    """Keep canvas-only images from consuming the durable saving namespace."""
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    root = tmp_path / "not-created"
    ticket = _issue(journal, root, "muted", persist=False)
    context = OutputNamingNumbers(journal).context(
        journal.read_ticket(ticket), width=100, height=80
    )
    assert context.run is None
    assert not root.exists()


def test_folder_image_counter_continues_existing_non_png_images(tmp_path: Path) -> None:
    """Preserve Substitute's folder-wide ordinal across supported companion formats."""
    root = tmp_path / "outputs"
    root.mkdir()
    (root / "result_04.jpg").write_bytes(b"fixture")
    (root / "result_02.png").write_bytes(b"fixture")
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    ticket = _issue(journal, root, "run-a", pattern="{source}_{image#}")
    numbers = OutputNamingNumbers(journal)
    assert (
        numbers.context(
            journal.read_ticket(ticket), width=100, height=80
        ).folder_image_number
        == 5
    )
    assert (
        numbers.context(
            journal.read_ticket(ticket), width=100, height=80
        ).folder_image_number
        == 6
    )


def test_run_folder_templates_and_folder_case_use_the_same_namespace(
    tmp_path: Path,
) -> None:
    """Keep folder tokens as folder text and allocate above existing numbered run folders."""
    root = tmp_path / "outputs"
    (root / "Example" / "007").mkdir(parents=True)
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    ticket = _issue(journal, root, "run-a", pattern="{workflow}/{run}/{source}")
    context = OutputNamingNumbers(journal).context(
        journal.read_ticket(ticket), width=100, height=80
    )
    assert context.run == 8
    assert [path.name for path in root.iterdir()] == ["Example"]


def test_run_tokens_inside_filename_continue_existing_numbering(tmp_path: Path) -> None:
    """Support migrated patterns whose run token does not begin the filename."""
    root = tmp_path / "outputs"
    root.mkdir()
    (root / "result_007.png").write_bytes(b"fixture")
    journal = SavingTicketJournal(tmp_path / "saving.sqlite")
    ticket = _issue(journal, root, "run-a", pattern="{source}_{run}")
    context = OutputNamingNumbers(journal).context(
        journal.read_ticket(ticket), width=100, height=80
    )
    assert context.run == 8
