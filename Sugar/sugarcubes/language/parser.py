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
            self._parse_line(items, bypassed=False)
        return SugarScriptParseResult(
            SugarScriptDocument(tuple(items)), tuple(self._diagnostics)
        )

    def _parse_line(self, items: list[SugarScriptItem], *, bypassed: bool) -> None:
        """Parse one statement and recover at its line boundary when it is malformed."""

        try:
            items.append(self._parse_statement(bypassed=bypassed))
        except _ParseFailure as failure:
            self._diagnostics.append(failure.diagnostic)
            self._recover_line()

    def _parse_comment(self, items: list[SugarScriptItem]) -> None:
        """Parse a run of bypassed lines or preserve one ordinary extension comment."""

        token = self._advance()
        if _bypass_payload(token) is None:
            self._keep_comment(token, items)
            return
        run = [token]
        # A bypassed Cube persists every physical line of its statements behind the marker, so
        # one statement, such as a multi-line string, may continue over the following lines.
        while (
            self._at(TokenKind.NEWLINE)
            and self._peek(1).kind is TokenKind.COMMENT
            and _bypass_payload(self._peek(1)) is not None
        ):
            self._advance()
            run.append(self._advance())
        if all(not _bypass_payload(comment) for comment in run):
            for comment in run:
                self._keep_comment(comment, items)
            return
        self._parse_bypassed_lines(run, items)

    def _keep_comment(self, token: Token, items: list[SugarScriptItem]) -> None:
        """Preserve one comment and report a malformed extension payload."""

        comment, diagnostic = decode_comment(token)
        items.append(comment)
        if diagnostic:
            self._diagnostics.append(diagnostic)

    def _parse_bypassed_lines(
        self,
        run: list[Token],
        items: list[SugarScriptItem],
    ) -> None:
        """Parse the statements a run of bypass lines holds, at their original locations."""

        payloads = [_bypass_payload(comment) or "" for comment in run]
        lexed = SugarScriptLexer("\n".join(payloads)).lex()
        relocation = _BypassRelocation(run, payloads)
        nested = SugarScriptParser.__new__(SugarScriptParser)
        nested._tokens = tuple(relocation.token(token) for token in lexed.tokens)
        nested._index = 0
        nested._diagnostics = [
            relocation.diagnostic(diagnostic) for diagnostic in lexed.diagnostics
        ]
        nested._expressions = SugarExpressionParser(nested)
        while not nested._at(TokenKind.EOF):
            if nested._at(TokenKind.NEWLINE):
                nested._advance()
            elif nested._at(TokenKind.COMMENT):
                # Companion comments of a bypassed statement, such as its model hash.
                nested._keep_comment(nested._advance(), items)
            else:
                nested._parse_line(items, bypassed=True)
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


_BYPASS = "bypass"


def _bypass_payload(comment: Token) -> str | None:
    """Return the text one bypass line persists, or `None` for any other comment.

    The writer prefixes each physical line with the marker and one space, so exactly that much is
    removed: whatever follows belongs to the persisted line, including its own indentation.
    """

    text = comment.value
    if not text.startswith(_BYPASS):
        return None
    rest = text[len(_BYPASS) :]
    if rest and not rest[0].isspace():
        return None
    return rest[1:] if rest.startswith(" ") else rest.strip()


class _BypassRelocation:
    """Map positions in the joined text of a bypass run back to its original source lines."""

    def __init__(self, run: list[Token], payloads: list[str]) -> None:
        """Record where each persisted line starts in the source and in the joined text."""

        self._origins: list[SourcePosition] = []
        self._starts: list[int] = []
        joined = 0
        for comment, payload in zip(run, payloads, strict=True):
            lead = len(comment.raw) - len(payload) if payload else len(comment.raw)
            start = comment.span.start
            self._origins.append(
                SourcePosition(start.offset + lead, start.line, start.column + lead)
            )
            self._starts.append(joined)
            joined += len(payload) + 1

    def position(self, position: SourcePosition) -> SourcePosition:
        """Relocate one position by the line of the run it falls on."""

        index = min(position.line, len(self._origins)) - 1
        origin = self._origins[index]
        within = position.offset - self._starts[index]
        return SourcePosition(
            origin.offset + within, origin.line, origin.column + within
        )

    def span(self, span: SourceSpan) -> SourceSpan:
        """Relocate both ends of one span."""

        return SourceSpan(self.position(span.start), self.position(span.end))

    def token(self, token: Token) -> Token:
        """Relocate one token."""

        return Token(token.kind, token.value, token.raw, self.span(token.span))

    def diagnostic(self, diagnostic: SugarScriptDiagnostic) -> SugarScriptDiagnostic:
        """Relocate one diagnostic."""

        return SugarScriptDiagnostic(
            diagnostic.code,
            diagnostic.message,
            diagnostic.severity,
            self.span(diagnostic.span),
        )
