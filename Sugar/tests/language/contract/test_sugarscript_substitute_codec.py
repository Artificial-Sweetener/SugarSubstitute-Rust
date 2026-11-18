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
"""Lock the value and name spellings shared with SugarSubstitute's persisted scripts."""

from __future__ import annotations

import pytest

from sugarcubes.language import SugarScriptLanguageService
from sugarcubes.language.substitute_codec import (
    encode_literal,
    encode_segment,
    encode_version_pin,
)
from sugarcubes.language.syntax import LiteralExpression, ListExpression, SetStatement


@pytest.mark.parametrize(
    ("value", "spelling"),
    [
        (None, "null"),
        (True, "True"),
        (False, "False"),
        (42, "42"),
        (12345678901234567890, "12345678901234567890"),
        (7.0, "7.0"),
        (0.35, "0.35"),
        (1e-05, "1e-05"),
        ([], "[]"),
        (["a b", "c"], '["a b", "c"]'),
        ("plain", '"plain"'),
        ('quote " and \\ slash', '"quote \\" and \\\\ slash"'),
        ("tab\there", '"tab\\there"'),
        ("café", '"café"'),
        ("two\nlines \\ raw", '"""two\nlines \\ raw"""'),
        ("carriage\r\nreturn", '"carriage\\r\\nreturn"'),
        ('ends with quote\n"', '"ends with quote\\n\\""'),
        ('holds """ inside\nit', '"holds \\"\\"\\" inside\\nit"'),
    ],
)
def test_values_are_spelled_as_substitute_persisted_them(
    value: object, spelling: str
) -> None:
    """Keep every scalar, list and string form byte-identical to the persisted format."""

    assert encode_literal(value) == spelling


@pytest.mark.parametrize(
    ("segment", "spelling"),
    [
        ("KSampler", "KSampler"),
        ("seed_mode", "seed_mode"),
        ("3D", "3D"),
        ("café", "café"),
        ("Text to Image", '"Text to Image"'),
        ("a.b", '"a.b"'),
        ("", '""'),
        ("_", '"_"'),
        ('say "hi"', '"say \\"hi\\""'),
    ],
)
def test_names_are_bare_only_when_alphanumeric(segment: str, spelling: str) -> None:
    """Quote every name the persisted grammar could not read bare."""

    assert encode_segment(segment) == spelling


def test_version_pins_are_bare_unless_they_carry_other_characters() -> None:
    """Match the pin spelling of persisted `use` lines."""

    assert encode_version_pin("1.2.0") == "1.2.0"
    assert encode_version_pin("1.0.0-beta") == '"1.0.0-beta"'


@pytest.mark.parametrize(
    "value",
    [True, False, 42, 7.0, 1e-05, "plain", 'q " \\ s', "two\nlines \\ raw", ["a", "b"]],
)
def test_every_spelling_reads_back_as_the_value_it_was_written_from(
    value: object,
) -> None:
    """A written value must survive this pack's own reader unchanged."""

    source = f"set {encode_segment('A cube')}.{encode_segment('n')}.v = {encode_literal(value)}\n"
    result = SugarScriptLanguageService().parse(source)

    assert result.diagnostics == ()
    statement = result.document.statements[0]
    assert isinstance(statement, SetStatement)
    assert statement.target.parts == ("A cube", "n", "v")
    expression = statement.value
    if isinstance(expression, ListExpression):
        read: object = [
            item.value
            for item in expression.items
            if isinstance(item, LiteralExpression)
        ]
    else:
        assert isinstance(expression, LiteralExpression)
        read = expression.value
    assert read == value and type(read) is type(value)
