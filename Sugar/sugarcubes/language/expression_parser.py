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
"""Parse SugarScript paths and expressions independently of statement grammar."""

from __future__ import annotations

from typing import Protocol

from .lexer import Token, TokenKind
from .source import SourceSpan, merge_spans
from .syntax import (
    BinaryExpression,
    ListExpression,
    LiteralExpression,
    NameExpression,
    PathExpression,
    RandomExpression,
    SugarExpression,
    SugarPath,
    SugarRange,
    UnaryExpression,
)


class ExpressionTokenStream(Protocol):
    """Expose the shared token cursor operations required by expression grammar."""

    def _at(self, kind: TokenKind) -> bool: ...

    def _at_any(self, kinds: tuple[TokenKind, ...]) -> bool: ...

    def _peek(self, distance: int = 0) -> Token: ...

    def _advance(self) -> Token: ...

    def _expect(self, kind: TokenKind, message: str) -> Token: ...

    def _expect_any(self, kinds: tuple[TokenKind, ...], message: str) -> Token: ...

    def _error(self, token: Token, message: str) -> Exception: ...


class SugarExpressionParser:
    """Own precedence, literal, list, path, wildcard, and range grammar."""

    def __init__(self, tokens: ExpressionTokenStream) -> None:
        """Share the statement parser's single token cursor."""

        self._tokens = tokens

    def parse_path(self, *, allow_wildcards: bool = False) -> SugarPath:
        """Parse a dotted path, optional alias range, and explicit wildcard segments."""

        kinds: tuple[TokenKind, ...] = (TokenKind.IDENTIFIER, TokenKind.STRING)
        if allow_wildcards:
            kinds += (TokenKind.STAR,)
        head = self._tokens._expect_any(kinds, "Expected path segment.")
        parts = [head.value]
        alias_range = (
            self._parse_range() if self._tokens._at(TokenKind.LEFT_BRACKET) else None
        )
        end_span = alias_range.span if alias_range else head.span
        while self._tokens._at(TokenKind.DOT):
            dot = self._tokens._advance()
            if not self._tokens._at_any(kinds):
                raise self._tokens._error(dot, "Expected path segment after '.'.")
            part = self._tokens._advance()
            parts.append(part.value)
            end_span = part.span
        return SugarPath(tuple(parts), alias_range, merge_spans(head.span, end_span))

    def parse_expression(self) -> SugarExpression:
        """Parse an expression using arithmetic precedence."""

        return self._parse_addition()

    def _parse_range(self) -> SugarRange:
        """Parse and validate one inclusive alias range."""

        left = self._tokens._expect(TokenKind.LEFT_BRACKET, "Expected '['.")
        start = self._tokens._expect(TokenKind.NUMBER, "Expected range start.")
        self._tokens._expect(TokenKind.DASH, "Expected '-' in range.")
        end = self._tokens._expect(TokenKind.NUMBER, "Expected range end.")
        right = self._tokens._expect(
            TokenKind.RIGHT_BRACKET, "Expected ']' after range."
        )
        if "." in start.value or "." in end.value:
            raise self._tokens._error(start, "Range bounds must be integers.")
        start_value, end_value = int(start.value), int(end.value)
        if start_value > end_value:
            raise self._tokens._error(start, "Range start cannot exceed range end.")
        return SugarRange(start_value, end_value, merge_spans(left.span, right.span))

    def _parse_addition(self) -> SugarExpression:
        """Parse left-associative addition and subtraction."""

        expression = self._parse_multiplication()
        while self._tokens._at_any((TokenKind.PLUS, TokenKind.DASH)):
            operator = self._tokens._advance()
            right = self._parse_multiplication()
            expression = BinaryExpression(
                expression,
                operator.value,
                right,
                merge_spans(expression.span, right.span),
            )
        return expression

    def _parse_multiplication(self) -> SugarExpression:
        """Parse left-associative multiplication and division."""

        expression = self._parse_unary()
        while self._tokens._at_any((TokenKind.STAR, TokenKind.SLASH)):
            operator = self._tokens._advance()
            right = self._parse_unary()
            expression = BinaryExpression(
                expression,
                operator.value,
                right,
                merge_spans(expression.span, right.span),
            )
        return expression

    def _parse_unary(self) -> SugarExpression:
        """Parse unary negation before primary expressions."""

        if self._tokens._at(TokenKind.DASH):
            operator = self._tokens._advance()
            operand = self._parse_unary()
            return UnaryExpression(
                "-", operand, merge_spans(operator.span, operand.span)
            )
        return self._parse_primary()

    def _parse_primary(self) -> SugarExpression:
        """Parse literals, lists, paths, names, random, and parenthesized expressions."""

        token = self._tokens._peek()
        if token.kind in {
            TokenKind.IDENTIFIER,
            TokenKind.STRING,
        } and self._tokens._peek(1).kind in {TokenKind.DOT, TokenKind.LEFT_BRACKET}:
            path = self.parse_path()
            return PathExpression(path, path.span)
        if token.kind is TokenKind.NUMBER:
            self._tokens._advance()
            value: int | float = (
                float(token.value) if "." in token.value else int(token.value)
            )
            return LiteralExpression(value, token.raw, token.span)
        if token.kind is TokenKind.STRING:
            self._tokens._advance()
            return LiteralExpression(token.value, token.raw, token.span)
        if token.kind is TokenKind.IDENTIFIER and token.value.lower() in {
            "true",
            "false",
            "null",
        }:
            self._tokens._advance()
            values: dict[str, object] = {"true": True, "false": False, "null": None}
            return LiteralExpression(values[token.value.lower()], token.raw, token.span)
        if token.kind is TokenKind.IDENTIFIER and token.value.lower() == "random":
            self._tokens._advance()
            return RandomExpression(token.span)
        if token.kind is TokenKind.LEFT_BRACKET:
            return self._parse_list()
        if token.kind is TokenKind.LEFT_PAREN:
            left = self._tokens._advance()
            expression = self.parse_expression()
            right = self._tokens._expect(
                TokenKind.RIGHT_PAREN, "Expected ')' after expression."
            )
            return _replace_expression_span(
                expression, merge_spans(left.span, right.span)
            )
        if token.kind in {TokenKind.IDENTIFIER, TokenKind.STRING}:
            self._tokens._advance()
            return NameExpression(token.value, token.span)
        raise self._tokens._error(token, "Expected expression.")

    def _parse_list(self) -> ListExpression:
        """Parse a nested comma-delimited list with an optional trailing comma."""

        left = self._tokens._advance()
        items: list[SugarExpression] = []
        if not self._tokens._at(TokenKind.RIGHT_BRACKET):
            while True:
                items.append(self.parse_expression())
                if not self._tokens._at(TokenKind.COMMA):
                    break
                self._tokens._advance()
                if self._tokens._at(TokenKind.RIGHT_BRACKET):
                    break
        right = self._tokens._expect(
            TokenKind.RIGHT_BRACKET, "Expected ']' after list."
        )
        return ListExpression(tuple(items), merge_spans(left.span, right.span))


def _replace_expression_span(
    expression: SugarExpression, span: SourceSpan
) -> SugarExpression:
    """Retain expression semantics while including explicit parentheses in its span."""

    if isinstance(expression, LiteralExpression):
        return LiteralExpression(expression.value, expression.raw, span)
    if isinstance(expression, ListExpression):
        return ListExpression(expression.items, span)
    if isinstance(expression, NameExpression):
        return NameExpression(expression.name, span)
    if isinstance(expression, RandomExpression):
        return RandomExpression(span)
    if isinstance(expression, PathExpression):
        return PathExpression(expression.path, span)
    if isinstance(expression, UnaryExpression):
        return UnaryExpression(expression.operator, expression.operand, span)
    return BinaryExpression(
        expression.left, expression.operator, expression.right, span
    )
