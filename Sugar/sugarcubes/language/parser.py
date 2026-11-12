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
"""Parse SugarScript tokens into recoverable, source-located typed syntax."""

from __future__ import annotations

from dataclasses import dataclass

from .expression_parser import SugarExpressionParser
from .extensions import decode_comment
from .lexer import SugarScriptLexer, Token, TokenKind
from .models import SugarScriptParseResult
from .source import (
    DiagnosticSeverity,
    SourcePosition,
    SourceSpan,
    SugarScriptDiagnostic,
    merge_spans,
)
from .syntax import (
    ConnectStatement,
    DisableStatement,
    EnableStatement,
    LetStatement,
    SetStatement,
    SugarScriptDocument,
    SugarScriptItem,
    SugarStatement,
    UseStatement,
)


@dataclass(frozen=True)
class _ParseFailure(Exception):
    """Carry one located parser diagnostic to the recovery loop."""

    diagnostic: SugarScriptDiagnostic


class SugarScriptParser:
    """Parse all valid statements and recover independently at line boundaries."""

    def __init__(self, source: str) -> None:
        """Lex source once and initialize the parser cursor."""

        lexed = SugarScriptLexer(source).lex()
        self._tokens = lexed.tokens
        self._index = 0
        self._diagnostics = list(lexed.diagnostics)
        self._expressions = SugarExpressionParser(self)

    def parse(self) -> SugarScriptParseResult:
        """Return a partial document plus every lexical, syntax, and extension issue."""

        items: list[SugarScriptItem] = []
        while not self._at(TokenKind.EOF):
            if self._at(TokenKind.NEWLINE):
                self._advance()
                continue
            if self._at(TokenKind.COMMENT):
                self._parse_comment(items)
                continue
            try:
                items.append(self._parse_statement(bypassed=False))
            except _ParseFailure as failure:
                self._diagnostics.append(failure.diagnostic)
                self._recover_line()
        return SugarScriptParseResult(
            SugarScriptDocument(tuple(items)), tuple(self._diagnostics)
        )

    def _parse_comment(self, items: list[SugarScriptItem]) -> None:
        """Parse bypassed statements or preserve ordinary extension comments."""

        token = self._advance()
        stripped = token.value.strip()
        if stripped == "bypass" or stripped.startswith("bypass "):
            statement_text = stripped.removeprefix("bypass").lstrip()
            if not statement_text:
                comment, diagnostic = decode_comment(token)
                items.append(comment)
                if diagnostic:
                    self._diagnostics.append(diagnostic)
                return
            self._parse_bypassed_statement(token, statement_text, items)
            return
        comment, diagnostic = decode_comment(token)
        items.append(comment)
        if diagnostic:
            self._diagnostics.append(diagnostic)

    def _parse_bypassed_statement(
        self,
        comment: Token,
        statement_text: str,
        items: list[SugarScriptItem],
    ) -> None:
        """Parse one persisted bypass statement while retaining its original location."""

        lexed = SugarScriptLexer(statement_text).lex()
        payload_offset = comment.raw.find(statement_text)
        shifted = tuple(
            _shift_token(token, comment.span.start, payload_offset)
            for token in lexed.tokens
        )
        nested = SugarScriptParser.__new__(SugarScriptParser)
        nested._tokens = shifted
        nested._index = 0
        nested._diagnostics = [
            _shift_diagnostic(diagnostic, comment.span.start, payload_offset)
            for diagnostic in lexed.diagnostics
        ]
        nested._expressions = SugarExpressionParser(nested)
        try:
            items.append(nested._parse_statement(bypassed=True))
        except _ParseFailure as failure:
            nested._diagnostics.append(failure.diagnostic)
        self._diagnostics.extend(nested._diagnostics)

    def _parse_statement(self, *, bypassed: bool) -> SugarStatement:
        """Dispatch one keyword-led line to its typed grammar production."""

        keyword = self._expect(
            TokenKind.IDENTIFIER, "Expected a SugarScript statement."
        )
        normalized = keyword.value.lower()
        statement: SugarStatement
        if normalized == "use":
            statement = self._parse_use(keyword, bypassed)
        elif normalized == "connect":
            source = self._expressions.parse_path()
            self._expect_keyword("to", "Expected 'to' in connect statement.")
            target = self._expressions.parse_path()
            statement = ConnectStatement(
                source, target, bypassed, merge_spans(keyword.span, target.span)
            )
        elif normalized == "set":
            target = self._expressions.parse_path(allow_wildcards=True)
            self._expect(TokenKind.EQUALS, "Expected '=' in set statement.")
            value = self._expressions.parse_expression()
            statement = SetStatement(
                target, value, bypassed, merge_spans(keyword.span, value.span)
            )
        elif normalized == "let":
            name = self._expect(TokenKind.IDENTIFIER, "Expected variable name.")
            self._expect(TokenKind.EQUALS, "Expected '=' in let statement.")
            value = self._expressions.parse_expression()
            statement = LetStatement(
                name.value, value, bypassed, merge_spans(keyword.span, value.span)
            )
        elif normalized in {"enable", "disable"}:
            target = self._expressions.parse_path()
            span = merge_spans(keyword.span, target.span)
            statement = (
                EnableStatement(target, bypassed, span)
                if normalized == "enable"
                else DisableStatement(target, bypassed, span)
            )
        else:
            raise self._error(keyword, f"Unknown statement '{keyword.value}'.")
        self._require_line_end()
        return statement

    def _parse_use(self, keyword: Token, bypassed: bool) -> UseStatement:
        """Parse a Cube reference and its optional version, flavor, alias, and repeat."""

        cube = self._expect_any(
            (TokenKind.IDENTIFIER, TokenKind.STRING),
            "Expected Cube identity after 'use'.",
        )
        version: str | None = None
        flavor: str | None = None
        alias: str | None = None
        repeat: int | None = None
        end = cube
        if self._at(TokenKind.AT):
            self._advance()
            version, end = self._parse_version()
        if self._at_keyword("with"):
            self._advance()
            flavor_token = self._expect_any(
                (TokenKind.IDENTIFIER, TokenKind.STRING),
                "Expected flavor name after 'with'.",
            )
            flavor, end = flavor_token.value, flavor_token
        if self._at_keyword("as"):
            self._advance()
            alias_token = self._expect_any(
                (TokenKind.IDENTIFIER, TokenKind.STRING), "Expected alias after 'as'."
            )
            alias, end = alias_token.value, alias_token
        if self._at_keyword("repeat"):
            self._advance()
            count = self._expect(TokenKind.NUMBER, "Expected integer repeat count.")
            if "." in count.value:
                raise self._error(count, "Repeat count must be an integer.")
            repeat = int(count.value)
            if repeat < 1:
                raise self._error(count, "Repeat count must be at least one.")
            end = count
        return UseStatement(
            cube.value,
            version,
            flavor,
            alias,
            repeat,
            bypassed,
            merge_spans(keyword.span, end.span),
        )

    def _parse_version(self) -> tuple[str, Token]:
        """Parse one quoted or dot-delimited version pin."""

        first = self._expect_any(
            (TokenKind.IDENTIFIER, TokenKind.STRING, TokenKind.NUMBER),
            "Expected version pin after '@'.",
        )
        if first.kind is TokenKind.STRING:
            return first.value, first
        parts = [first.value]
        end = first
        while self._at(TokenKind.DOT):
            dot = self._advance()
            if not self._at_any((TokenKind.IDENTIFIER, TokenKind.NUMBER)):
                raise self._error(dot, "Expected version segment after '.'.")
            end = self._advance()
            parts.append(end.value)
        return ".".join(parts), end

    def _require_line_end(self) -> None:
        """Reject extra tokens instead of silently treating them as a new statement."""

        if not self._at_any((TokenKind.NEWLINE, TokenKind.EOF, TokenKind.COMMENT)):
            raise self._error(self._peek(), "Expected the end of the statement.")
        if self._at(TokenKind.NEWLINE):
            self._advance()

    def _recover_line(self) -> None:
        """Skip only the malformed physical statement and resume at the next line."""

        while not self._at_any((TokenKind.NEWLINE, TokenKind.EOF)):
            self._advance()
        if self._at(TokenKind.NEWLINE):
            self._advance()

    def _expect_keyword(self, value: str, message: str) -> Token:
        """Consume one case-insensitive keyword represented as an identifier."""

        token = self._peek()
        if token.kind is not TokenKind.IDENTIFIER or token.value.lower() != value:
            raise self._error(token, message)
        return self._advance()

    def _expect(self, kind: TokenKind, message: str) -> Token:
        """Consume one required token kind."""

        token = self._peek()
        if token.kind is not kind:
            raise self._error(token, message)
        return self._advance()

    def _expect_any(self, kinds: tuple[TokenKind, ...], message: str) -> Token:
        """Consume one token from a required set."""

        token = self._peek()
        if token.kind not in kinds:
            raise self._error(token, message)
        return self._advance()

    def _at_keyword(self, value: str) -> bool:
        """Return whether the current token is a case-insensitive keyword."""

        token = self._peek()
        return token.kind is TokenKind.IDENTIFIER and token.value.lower() == value

    def _at(self, kind: TokenKind) -> bool:
        """Return whether the current token has one kind."""

        return self._peek().kind is kind

    def _at_any(self, kinds: tuple[TokenKind, ...]) -> bool:
        """Return whether the current token has any requested kind."""

        return self._peek().kind in kinds

    def _peek(self, distance: int = 0) -> Token:
        """Read a token relative to the parser cursor."""

        index = min(self._index + distance, len(self._tokens) - 1)
        return self._tokens[index]

    def _advance(self) -> Token:
        """Consume and return the current token."""

        token = self._peek()
        self._index = min(self._index + 1, len(self._tokens))
        return token

    @staticmethod
    def _error(token: Token, message: str) -> _ParseFailure:
        """Create one stable parser error at the offending token."""

        return _ParseFailure(
            SugarScriptDiagnostic(
                "sugarscript.parse.unexpected_token",
                message,
                DiagnosticSeverity.ERROR,
                token.span,
            )
        )


def _shift_token(token: Token, origin: SourcePosition, column_offset: int) -> Token:
    """Relocate an inline bypass token into its enclosing comment source span."""

    return Token(
        token.kind,
        token.value,
        token.raw,
        _shift_span(token.span, origin, column_offset),
    )


def _shift_diagnostic(
    diagnostic: SugarScriptDiagnostic,
    origin: SourcePosition,
    column_offset: int,
) -> SugarScriptDiagnostic:
    """Relocate an inline bypass diagnostic into the enclosing source line."""

    return SugarScriptDiagnostic(
        diagnostic.code,
        diagnostic.message,
        diagnostic.severity,
        _shift_span(diagnostic.span, origin, column_offset),
    )


def _shift_span(
    span: SourceSpan, origin: SourcePosition, column_offset: int
) -> SourceSpan:
    """Shift a single-line nested span into its original comment coordinates."""

    def shift(position: SourcePosition) -> SourcePosition:
        return SourcePosition(
            origin.offset + column_offset + position.offset,
            origin.line + position.line - 1,
            origin.column + column_offset + position.column - 1,
        )

    return SourceSpan(shift(span.start), shift(span.end))
