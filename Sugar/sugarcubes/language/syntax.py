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
"""Define the immutable SugarScript syntax tree independently of execution."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping, TypeAlias

from .source import SourceSpan


@dataclass(frozen=True)
class SugarRange:
    """Represent an inclusive alias range."""

    start: int
    end: int
    span: SourceSpan


@dataclass(frozen=True)
class SugarPath:
    """Represent a dotted symbol path with an optional alias range."""

    parts: tuple[str, ...]
    alias_range: SugarRange | None
    span: SourceSpan


@dataclass(frozen=True)
class LiteralExpression:
    """Represent a decoded scalar literal and its original lexical text."""

    value: object
    raw: str
    span: SourceSpan


@dataclass(frozen=True)
class ListExpression:
    """Represent an ordered SugarScript literal list."""

    items: tuple[SugarExpression, ...]
    span: SourceSpan


@dataclass(frozen=True)
class NameExpression:
    """Represent a local variable reference."""

    name: str
    span: SourceSpan


@dataclass(frozen=True)
class RandomExpression:
    """Represent the deferred random-value keyword."""

    span: SourceSpan


@dataclass(frozen=True)
class PathExpression:
    """Represent a reference to another Cube field or endpoint."""

    path: SugarPath
    span: SourceSpan


@dataclass(frozen=True)
class UnaryExpression:
    """Represent one prefix arithmetic operation."""

    operator: str
    operand: SugarExpression
    span: SourceSpan


@dataclass(frozen=True)
class BinaryExpression:
    """Represent one precedence-aware arithmetic operation."""

    left: SugarExpression
    operator: str
    right: SugarExpression
    span: SourceSpan


SugarExpression: TypeAlias = (
    LiteralExpression
    | ListExpression
    | NameExpression
    | RandomExpression
    | PathExpression
    | UnaryExpression
    | BinaryExpression
)


@dataclass(frozen=True)
class UseStatement:
    """Declare one or more instances of a Cube artifact."""

    cube_id: str
    version_pin: str | None
    flavor: str | None
    alias: str | None
    repeat: int | None
    bypassed: bool
    span: SourceSpan


@dataclass(frozen=True)
class ConnectStatement:
    """Connect two declared Cube boundary paths."""

    source: SugarPath
    target: SugarPath
    bypassed: bool
    span: SourceSpan


@dataclass(frozen=True)
class SetStatement:
    """Assign an expression to an exact or wildcard field path."""

    target: SugarPath
    value: SugarExpression
    bypassed: bool
    span: SourceSpan


@dataclass(frozen=True)
class LetStatement:
    """Bind a reusable expression value within a script."""

    name: str
    value: SugarExpression
    bypassed: bool
    span: SourceSpan


@dataclass(frozen=True)
class EnableStatement:
    """Request an authored node to participate in execution."""

    target: SugarPath
    bypassed: bool
    span: SourceSpan


@dataclass(frozen=True)
class DisableStatement:
    """Request an authored node to be bypassed during execution."""

    target: SugarPath
    bypassed: bool
    span: SourceSpan


SugarStatement: TypeAlias = (
    UseStatement
    | ConnectStatement
    | SetStatement
    | LetStatement
    | EnableStatement
    | DisableStatement
)


@dataclass(frozen=True)
class SugarScriptComment:
    """Preserve a plain, provenance, or supported extension comment."""

    raw: str
    extension_name: str | None
    extension_payload: Mapping[str, object] | str | None
    span: SourceSpan


SugarScriptItem: TypeAlias = SugarStatement | SugarScriptComment


@dataclass(frozen=True)
class SugarScriptDocument:
    """Hold source-ordered typed statements and losslessly preserved comments."""

    items: tuple[SugarScriptItem, ...]

    @property
    def statements(self) -> tuple[SugarStatement, ...]:
        """Return executable language statements in source order."""

        return tuple(
            item for item in self.items if not isinstance(item, SugarScriptComment)
        )

    @property
    def comments(self) -> tuple[SugarScriptComment, ...]:
        """Return preserved comments in source order."""

        return tuple(
            item for item in self.items if isinstance(item, SugarScriptComment)
        )
