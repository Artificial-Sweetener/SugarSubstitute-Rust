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
"""Lock the supported Sugar DSL surface before SugarScript implementation."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class _LanguageCase:
    """Describe one compatibility input and its observable statement sequence."""

    name: str
    source: str
    statements: tuple[str, ...]


CASES = (
    _LanguageCase(
        "aliases_and_connections",
        'use "source"@2.0.0 as "Text to Image"\n'
        'use "target" as Out\n'
        'connect "Text to Image".output.image to Out.input.image\n',
        ("use", "use", "connect"),
    ),
    _LanguageCase(
        "values_selectors_and_activation",
        'use "image" as Cube repeat 2\n'
        "let base_steps = 25\n"
        "set *.KSampler.cfg = 7.5\n"
        "set cube1.sampler.steps = base_steps\n"
        "disable cube2.checkpoint\n"
        "enable cube2.checkpoint\n",
        ("use", "let", "set", "set", "disable", "enable"),
    ),
    _LanguageCase(
        "typed_literals",
        'use "types" as T\n'
        'set T.node.text = "café"\n'
        "set T.node.enabled = true\n"
        "set T.node.optional = null\n"
        'set T.node.batch = ["first.png", [1, true], null,]\n'
        "set T.node.seed = random\n",
        ("use", "set", "set", "set", "set", "set"),
    ),
    _LanguageCase(
        "quoted_whole_node_links",
        'use "source" as "Text to Image"\n'
        'use "target" as "Diffusion Upscale"\n'
        'set "Diffusion Upscale"."positive prompt" = '
        '"Text to Image"."positive prompt"\n',
        ("use", "use", "set"),
    ),
)


def test_corpus_covers_every_supported_statement_and_literal_family() -> None:
    """Make the port account for grammar features already accepted by Sugar DSL."""

    statements = {statement for case in CASES for statement in case.statements}
    combined_source = "\n".join(case.source for case in CASES)

    assert statements == {"use", "connect", "set", "let", "enable", "disable"}
    assert all(token in combined_source for token in ("true", "null", "random", "["))


def test_each_characterization_records_one_statement_per_source_line() -> None:
    """Anchor source-line accounting used by later diagnostic spans."""

    for case in CASES:
        source_lines = tuple(line for line in case.source.splitlines() if line.strip())
        assert len(source_lines) == len(case.statements), case.name
