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
"""Prove captured tickets drive real image saves, muted previews and reload-safe references."""

from dataclasses import replace
from datetime import datetime
from pathlib import Path
import json

from PIL import Image

from sugarcubes.output_saving.domain.preferences import (
    JpegPreferences,
    OutputSavingPreferences,
)
from sugarcubes.output_saving.domain.run_plan import SavingRun, SavingSource, SourceKind
from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.managed_image_writer import (
    ManagedImageWriter,
)
from sugarcubes.output_saving.infrastructure.naming_numbers import OutputNamingNumbers
from sugarcubes.output_saving.infrastructure.ticket_journal import SavingTicketJournal


def test_three_images_use_one_run_and_retain_exact_per_image_metadata(
    tmp_path: Path,
) -> None:
    """Exercise the complete configured persistence path using distinguishable batch pixels."""
    journal = SavingTicketJournal(tmp_path / "state" / "tickets.sqlite")
    registry = ManagedArtifactRegistry(tmp_path / "state" / "artifacts.sqlite")
    run = SavingRun(
        "run-a",
        str(tmp_path / "output"),
        "Example",
        datetime(2026, 5, 1),
        replace(OutputSavingPreferences(), jpeg=JpegPreferences(enabled=True)),
        (("workflow", '{"nodes":[]}'),),
    )
    identity = journal.issue_run(
        run,
        (
            SavingSource(
                "sink",
                "Result",
                SourceKind.CUBE,
                cube_id="a",
                cube_number=1,
                persist=True,
            ),
        ),
    )[0]
    writer = ManagedImageWriter(
        numbers=OutputNamingNumbers(journal),
        artifacts=registry,
        temporary_root=tmp_path / "temp",
    )
    refs = [
        writer.write(
            Image.new("RGB", (4, 3), (index, 0, 0)), journal.read_ticket(identity)
        )
        for index in range(3)
    ]
    assert len({ref.artifact_id for ref in refs}) == 3
    assert [ref.filename for ref in refs] == [
        "001_01_example_result.png",
        "001_01_example_result_002.png",
        "001_01_example_result_003.png",
    ]
    for index, ref in enumerate(refs):
        path = registry.resolve(ref.artifact_id, ref.filename, "output")
        assert path is not None
        with Image.open(path) as decoded:
            assert decoded.getpixel((0, 0)) == (index, 0, 0)
            assert decoded.info["workflow"] == '{"nodes":[]}'
            identity_metadata: object = json.loads(decoded.info["sugarcubes_output"])
            assert isinstance(identity_metadata, dict)
            assert identity_metadata["image_index"] == index + 1
            assert identity_metadata["run_id"] == "run-a"
            assert identity_metadata["source_node_id"] == "sink"
            assert identity_metadata["run_number"] == 1
        assert ref.companion_filename is not None
        assert (
            registry.resolve(ref.artifact_id, ref.companion_filename, "output")
            is not None
        )
    assert not (tmp_path / "temp").exists()


def test_muted_cube_keeps_preview_without_durable_outputs_or_companions(
    tmp_path: Path,
) -> None:
    """Retain canvas-visible PNGs and recipe data while suppressing configured durable saving."""
    journal = SavingTicketJournal(tmp_path / "tickets.sqlite")
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    run = SavingRun(
        "muted",
        str(tmp_path / "output"),
        "Example",
        datetime(2026, 5, 1),
        replace(OutputSavingPreferences(), jpeg=JpegPreferences(enabled=True)),
    )
    identity = journal.issue_run(
        run, (SavingSource("sink", "Result", SourceKind.CUBE, persist=False),)
    )[0]
    writer = ManagedImageWriter(
        numbers=OutputNamingNumbers(journal),
        artifacts=registry,
        temporary_root=tmp_path / "temp",
    )
    ref = writer.write(Image.new("RGB", (4, 3)), journal.read_ticket(identity))
    assert ref.type == "temp"
    assert ref.companion_filename is None
    assert not (tmp_path / "output").exists()
    assert registry.resolve(ref.artifact_id, ref.filename, "temp") is not None


def test_registered_reference_survives_reopening_and_rejects_other_paths(
    tmp_path: Path,
) -> None:
    """Serve only exact registered image names from previously approved roots after reload."""
    root = tmp_path / "output"
    root.mkdir()
    png = root / "result.png"
    Image.new("RGB", (2, 2)).save(png)
    state = tmp_path / "artifacts.sqlite"
    ref = ManagedArtifactRegistry(state).register(
        root=root, png=png, jpeg=None, saved=True, width=2, height=2
    )
    registry = ManagedArtifactRegistry(state)
    assert registry.resolve(ref.artifact_id, ref.filename, "output") == png
    assert registry.resolve(ref.artifact_id, "../result.png", "output") is None
    assert registry.resolve(ref.artifact_id, ref.filename, "temp") is None
    assert registry.resolve("not-a-token", ref.filename, "output") is None
    png.unlink()
    assert registry.resolve(ref.artifact_id, ref.filename, "output") is None


def test_disabled_metadata_is_not_written_even_when_a_workflow_snapshot_exists(
    tmp_path: Path,
) -> None:
    """Respect Comfy's explicit metadata privacy setting for canonical and temporary outputs."""
    journal = SavingTicketJournal(tmp_path / "tickets.sqlite")
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    run = SavingRun(
        "private",
        str(tmp_path / "output"),
        "Example",
        datetime(2026, 5, 1),
        OutputSavingPreferences(),
        (("workflow", "private workflow"),),
        metadata_enabled=False,
    )
    identity = journal.issue_run(
        run, (SavingSource("sink", "Result", SourceKind.CUBE, persist=True),)
    )[0]
    writer = ManagedImageWriter(
        numbers=OutputNamingNumbers(journal),
        artifacts=registry,
        temporary_root=tmp_path / "temp",
    )
    ref = writer.write(Image.new("RGB", (4, 3)), journal.read_ticket(identity))
    path = registry.resolve(ref.artifact_id, ref.filename, "output")
    assert path is not None
    with Image.open(path) as image:
        assert "workflow" not in image.info
        assert "sugarcubes_output" not in image.info
