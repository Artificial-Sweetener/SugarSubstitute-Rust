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
"""Seed durable output counters from existing files using Substitute naming namespaces."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
import re

from ..domain.path_pattern import (
    OutputNamingContext,
    render_components,
    run_bucket_pattern,
    validate_pattern,
)
from ..domain.run_plan import SavingTicket, SourceKind
from .ticket_journal import SavingTicketJournal

_IMAGE_SUFFIXES = frozenset({".png", ".jpg", ".jpeg", ".webp", ".bmp"})
_PLACEHOLDER = re.compile(r"\{([^{}]+)\}")
_UNSAFE = re.compile(r'[<>:"/\\|?*\x00-\x1f]+')


class OutputNamingNumbers:
    """Own filesystem counter bootstrapping while the journal owns reservations."""

    def __init__(self, journal: SavingTicketJournal) -> None:
        """Receive the authoritative transaction owner instead of retaining local counters."""
        self._journal = journal

    def context(
        self, ticket: SavingTicket, *, width: int, height: int
    ) -> OutputNamingContext:
        """Allocate source, run and optional folder ordinals only for an actual output image."""
        run, source = ticket.run, ticket.source
        index = self._journal.next_source_index(ticket.ticket_id)
        context = OutputNamingContext(
            workflow=run.workflow,
            source=source.label,
            cube=source.cube_alias,
            cube_number=source.cube_number,
            seed=(
                run.generation_seed
                if source.kind is SourceKind.CUBE and run.generation_seed is not None
                else source.seed
            ),
            started_at=run.started_at,
            width=width,
            height=height,
            index=index,
            set_index=index,
            node=source.label,
            node_id=source.node_id,
        )
        if not source.persist:
            return context
        root = Path(run.root).resolve()
        pattern = run.preferences.organization.path_pattern
        bucket_pattern = run_bucket_pattern(pattern)
        bucket = root
        if bucket_pattern is not None:
            bucket = root.joinpath(
                *render_components(f"{bucket_pattern}/output", context)[:-1]
            ).resolve()
        self._require_contained(root, bucket)
        number = self._journal.run_number(
            run.run_id, _bucket_key(bucket), lambda: _next_run_number(bucket, pattern)
        )
        context = replace(context, run=number)
        if "{image#}" in pattern:
            directory = root.joinpath(
                *render_components(pattern, context)[:-1]
            ).resolve()
            self._require_contained(root, directory)
            ordinal = self._journal.image_number(
                _bucket_key(directory), lambda: _next_image_number(directory, pattern)
            )
            context = replace(context, folder_image_number=ordinal)
        return context

    @staticmethod
    def _require_contained(root: Path, directory: Path) -> None:
        """Reject symlink escapes before scanning external files or reserving their namespace."""
        if not directory.is_relative_to(root):
            raise ValueError("Output counter namespace escapes the configured folder.")


def _bucket_key(directory: Path) -> str:
    """Preserve Substitute's stable case-folded directory namespace identity."""
    return str(directory).replace("\\", "/").casefold()


def _next_run_number(directory: Path, pattern: str) -> int:
    """Seed the committed namespace from the actual configured run-token component."""
    if not directory.is_dir():
        return 1
    parts = validate_pattern(pattern)
    filename = "{run}" in parts[-1]
    component = (
        parts[-1]
        if filename
        else next((part for part in parts[:-1] if "{run}" in part), "{run}_{source}")
    )
    expression = _ordinal_expression(component, "run", filename=filename)
    maximum = 0
    for path in directory.iterdir():
        if path.is_file() and path.suffix.lower() != ".png":
            continue
        match = expression.fullmatch(path.stem if path.is_file() else path.name)
        if match is not None:
            maximum = max(maximum, int(match.group("ordinal")))
    return maximum + 1


def _next_image_number(directory: Path, pattern: str) -> int:
    """Seed folder-wide image ordinals using the same filename token extraction as Substitute."""
    if not directory.is_dir():
        return 1
    expression = _ordinal_expression(
        validate_pattern(pattern)[-1], "image#", filename=True
    )
    maximum = 0
    for path in directory.iterdir():
        if not path.is_file() or path.suffix.lower() not in _IMAGE_SUFFIXES:
            continue
        result = expression.fullmatch(path.stem)
        if result is not None:
            maximum = max(maximum, int(result.group("ordinal")))
    return maximum + 1


def _ordinal_expression(
    component: str, token: str, *, filename: bool
) -> re.Pattern[str]:
    """Extract repeated numbering tokens while retaining portable template literal normalization."""
    stem = component.strip(" ._")
    parts: list[str] = []
    cursor = 0
    found_ordinal = False
    for match in _PLACEHOLDER.finditer(stem):
        parts.append(
            re.escape(
                _normalized_literal(stem[cursor : match.start()], filename=filename)
            )
        )
        if match.group(1) == token:
            parts.append(r"(?P=ordinal)" if found_ordinal else r"(?P<ordinal>\d+)")
            found_ordinal = True
        else:
            parts.append(r".*?")
        cursor = match.end()
    parts.append(re.escape(_normalized_literal(stem[cursor:], filename=filename)))
    return re.compile(f"^{''.join(parts)}$", re.I)


def _normalized_literal(value: str, *, filename: bool) -> str:
    """Match Substitute's component literals without trimming their token separators."""
    text = _UNSAFE.sub("_", value.lower() if filename else value)
    if filename:
        text = re.sub(r"\s+", "_", text)
    return re.sub(r"_+", "_", text)
