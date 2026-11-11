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
"""Evaluate SugarScript expressions against resolved instance values."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import TypeAlias

from .syntax import (
    ListExpression,
    LiteralExpression,
    NameExpression,
    PathExpression,
    RandomExpression,
    SugarExpression,
    UnaryExpression,
)

RANDOM_VALUE: Mapping[str, str] = {"$sugarscript": "random"}
Number: TypeAlias = int | float


def evaluate_expression(
    expression: SugarExpression,
    variables: Mapping[str, object],
    resolve_path: Callable[[tuple[str, ...]], object],
) -> object:
    """Evaluate one typed expression without filesystem, catalog, or runtime access."""

    if isinstance(expression, LiteralExpression):
        return expression.value
    if isinstance(expression, ListExpression):
        return [
            evaluate_expression(item, variables, resolve_path)
            for item in expression.items
        ]
    if isinstance(expression, NameExpression):
        if expression.name not in variables:
            raise ValueError(f"Unknown variable '{expression.name}'.")
        return variables[expression.name]
    if isinstance(expression, RandomExpression):
        return dict(RANDOM_VALUE)
    if isinstance(expression, PathExpression):
        if expression.path.alias_range is not None:
            raise ValueError("Range references are not valid in expressions.")
        return resolve_path(expression.path.parts)
    if isinstance(expression, UnaryExpression):
        value = evaluate_expression(expression.operand, variables, resolve_path)
        return -_read_number(value, "unary '-' expression")
    left = evaluate_expression(expression.left, variables, resolve_path)
    right = evaluate_expression(expression.right, variables, resolve_path)
    return _apply_binary(expression.operator, left, right)


def _apply_binary(operator: str, left: object, right: object) -> object:
    """Apply one validated arithmetic operator."""

    if operator == "+" and isinstance(left, str) and isinstance(right, str):
        return left + right
    left_number = _read_number(left, f"'{operator}' expression")
    right_number = _read_number(right, f"'{operator}' expression")
    if operator == "+":
        return left_number + right_number
    if operator == "-":
        return left_number - right_number
    if operator == "*":
        return left_number * right_number
    if operator == "/":
        if right_number == 0:
            raise ValueError("Division by zero.")
        return left_number / right_number
    raise ValueError(f"Unsupported operator '{operator}'.")


def _read_number(value: object, context: str) -> Number:
    """Return a numeric value while rejecting booleans and other values."""

    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"Expected a number for {context}.")
    return value
