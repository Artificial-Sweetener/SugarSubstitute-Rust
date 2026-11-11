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
"""Render typed SugarScript deterministically without workflow or UI knowledge."""

from __future__ import annotations

import json
import re

from .models import SugarScriptRenderResult, SugarScriptSourceMapEntry
from .syntax import (
    ConnectStatement,
    DisableStatement,
    EnableStatement,
    LetStatement,
    ListExpression,
    LiteralExpression,
    NameExpression,
    PathExpression,
    RandomExpression,
    SetStatement,
    SugarExpression,
    SugarPath,
    SugarScriptComment,
    SugarScriptDocument,
    SugarStatement,
    UnaryExpression,
    UseStatement,
)

_IDENTIFIER_PATTERN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def render_sugarscript(document: SugarScriptDocument) -> SugarScriptRenderResult:
    """Render one source-ordered document and map each output line to its origin."""

    lines: list[str] = []
    source_map: list[SugarScriptSourceMapEntry] = []
    for item in document.items:
        line = (
            item.raw
            if isinstance(item, SugarScriptComment)
            else _render_statement(item)
        )
        if not isinstance(item, SugarScriptComment) and item.bypassed:
            line = f"# bypass {line}"
        lines.append(line)
        source_map.append(SugarScriptSourceMapEntry(len(lines), item.span))
    source = "\n".join(lines)
    if lines:
        source += "\n"
    return SugarScriptRenderResult(source, tuple(source_map))


def _render_statement(statement: SugarStatement) -> str:
    """Render one statement with canonical keyword and spacing rules."""

    if isinstance(statement, UseStatement):
        parts = ["use", _quote(statement.cube_id)]
        if statement.version_pin is not None:
            parts[-1] += f"@{_render_version(statement.version_pin)}"
        if statement.flavor is not None:
            parts.extend(("with", _quote_if_needed(statement.flavor)))
        if statement.alias is not None:
            parts.extend(("as", _quote_if_needed(statement.alias)))
        if statement.repeat is not None:
            parts.extend(("repeat", str(statement.repeat)))
        return " ".join(parts)
    if isinstance(statement, ConnectStatement):
        return f"connect {_render_path(statement.source)} to {_render_path(statement.target)}"
    if isinstance(statement, SetStatement):
        return f"set {_render_path(statement.target)} = {_render_expression(statement.value)}"
    if isinstance(statement, LetStatement):
        return f"let {statement.name} = {_render_expression(statement.value)}"
    if isinstance(statement, EnableStatement):
        return f"enable {_render_path(statement.target)}"
    if isinstance(statement, DisableStatement):
        return f"disable {_render_path(statement.target)}"
    raise TypeError(f"Unsupported SugarScript statement: {type(statement).__name__}")


def _render_path(path: SugarPath) -> str:
    """Render a path without losing quoted labels or wildcard intent."""

    parts = ["*" if part == "*" else _quote_if_needed(part) for part in path.parts]
    if path.alias_range is not None:
        parts[0] += f"[{path.alias_range.start}-{path.alias_range.end}]"
    return ".".join(parts)


def _render_expression(expression: SugarExpression, parent_precedence: int = 0) -> str:
    """Render an expression with the minimum deterministic parentheses."""

    if isinstance(expression, LiteralExpression):
        if isinstance(expression.value, str):
            return _quote(expression.value)
        if expression.value is True:
            return "true"
        if expression.value is False:
            return "false"
        if expression.value is None:
            return "null"
        return str(expression.value)
    if isinstance(expression, ListExpression):
        return (
            "[" + ", ".join(_render_expression(item) for item in expression.items) + "]"
        )
    if isinstance(expression, NameExpression):
        return expression.name
    if isinstance(expression, RandomExpression):
        return "random"
    if isinstance(expression, PathExpression):
        return _render_path(expression.path)
    if isinstance(expression, UnaryExpression):
        rendered = f"{expression.operator}{_render_expression(expression.operand, 3)}"
        return f"({rendered})" if parent_precedence > 3 else rendered
    precedence = 1 if expression.operator in {"+", "-"} else 2
    rendered = (
        f"{_render_expression(expression.left, precedence)} {expression.operator} "
        f"{_render_expression(expression.right, precedence + 1)}"
    )
    return f"({rendered})" if parent_precedence > precedence else rendered


def _render_version(version: str) -> str:
    """Quote version pins only when the legacy unquoted grammar cannot read them."""

    if all(
        _IDENTIFIER_PATTERN.fullmatch(part) or part.isdigit()
        for part in version.split(".")
    ):
        return version
    return _quote(version)


def _quote_if_needed(value: str) -> str:
    """Render a simple symbol plainly and every other value as a JSON string."""

    return value if _IDENTIFIER_PATTERN.fullmatch(value) else _quote(value)


def _quote(value: str) -> str:
    """Return a stable Unicode-preserving double-quoted string."""

    return json.dumps(value, ensure_ascii=False)
