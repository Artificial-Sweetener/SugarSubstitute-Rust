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
"""Tokenize SugarScript while retaining exact source locations and comments."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum, auto

from .source import (
    DiagnosticSeverity,
    SourcePosition,
    SourceSpan,
    SugarScriptDiagnostic,
)


class TokenKind(Enum):
    """Identify lexical elements consumed by the SugarScript parser."""

    IDENTIFIER = auto()
    STRING = auto()
    NUMBER = auto()
    COMMENT = auto()
    NEWLINE = auto()
    DOT = auto()
    COMMA = auto()
    EQUALS = auto()
    STAR = auto()
    PLUS = auto()
    SLASH = auto()
    AT = auto()
    LEFT_BRACKET = auto()
    RIGHT_BRACKET = auto()
    DASH = auto()
    LEFT_PAREN = auto()
    RIGHT_PAREN = auto()
    EOF = auto()


@dataclass(frozen=True)
class Token:
    """Represent one decoded token and its original lexical text."""

    kind: TokenKind
    value: str
    raw: str
    span: SourceSpan


@dataclass(frozen=True)
class LexResult:
    """Return a complete token stream and recoverable lexical diagnostics."""

    tokens: tuple[Token, ...]
    diagnostics: tuple[SugarScriptDiagnostic, ...]


_SYMBOLS = {
    ".": TokenKind.DOT,
    ",": TokenKind.COMMA,
    "=": TokenKind.EQUALS,
    "*": TokenKind.STAR,
    "+": TokenKind.PLUS,
    "/": TokenKind.SLASH,
    "@": TokenKind.AT,
    "[": TokenKind.LEFT_BRACKET,
    "]": TokenKind.RIGHT_BRACKET,
    "-": TokenKind.DASH,
    "(": TokenKind.LEFT_PAREN,
    ")": TokenKind.RIGHT_PAREN,
}


class SugarScriptLexer:
    """Scan one SugarScript source without raising on user-authored syntax errors."""

    def __init__(self, source: str) -> None:
        """Initialize a source cursor at the first code point."""

        self._source = source
        self._offset = 0
        self._line = 1
        self._column = 1
        self._diagnostics: list[SugarScriptDiagnostic] = []

    def lex(self) -> LexResult:
        """Return every token plus deterministic diagnostics for invalid characters."""

        tokens: list[Token] = []
        while not self._at_end:
            character = self._peek()
            if character in " \t":
                self._advance()
            elif character in "\r\n":
                tokens.append(self._read_newline())
            elif character == "#":
                tokens.append(self._read_comment())
            elif character in {'"', "'"}:
                tokens.append(self._read_string())
            elif character.isdigit():
                tokens.append(self._read_number())
            elif character.isalpha() or character == "_":
                tokens.append(self._read_identifier())
            elif character in _SYMBOLS:
                tokens.append(self._read_symbol())
            else:
                start = self._position
                invalid = self._advance()
                self._diagnostics.append(
                    SugarScriptDiagnostic(
                        "sugarscript.lex.unexpected_character",
                        f"Unexpected character {invalid!r}.",
                        DiagnosticSeverity.ERROR,
                        SourceSpan(start, self._position),
                    )
                )
        eof = self._position
        tokens.append(Token(TokenKind.EOF, "", "", SourceSpan(eof, eof)))
        return LexResult(tuple(tokens), tuple(self._diagnostics))

    @property
    def _at_end(self) -> bool:
        """Return whether all source code points have been consumed."""

        return self._offset >= len(self._source)

    @property
    def _position(self) -> SourcePosition:
        """Return the current half-open source position."""

        return SourcePosition(self._offset, self._line, self._column)

    def _peek(self, distance: int = 0) -> str:
        """Read one code point without changing cursor state."""

        offset = self._offset + distance
        return self._source[offset] if offset < len(self._source) else ""

    def _advance(self) -> str:
        """Consume one code point and update human-readable coordinates."""

        character = self._source[self._offset]
        self._offset += 1
        if character == "\n":
            self._line += 1
            self._column = 1
        else:
            self._column += 1
        return character

    def _read_newline(self) -> Token:
        """Normalize CR, LF, and CRLF to one newline token."""

        start = self._position
        raw = self._advance()
        if raw == "\r" and self._peek() == "\n":
            raw += self._advance()
        return Token(TokenKind.NEWLINE, "\n", raw, SourceSpan(start, self._position))

    def _read_comment(self) -> Token:
        """Retain a complete comment for extension and provenance interpretation."""

        start = self._position
        characters: list[str] = []
        while not self._at_end and self._peek() not in "\r\n":
            characters.append(self._advance())
        raw = "".join(characters)
        return Token(
            TokenKind.COMMENT, raw[1:].lstrip(), raw, SourceSpan(start, self._position)
        )

    def _read_string(self) -> Token:
        """Decode quoted or triple-quoted strings while retaining the lexical form."""

        start = self._position
        quote = self._peek()
        triple = quote == '"' and self._peek(1) == '"' and self._peek(2) == '"'
        delimiter = '"""' if triple else quote
        for _ in delimiter:
            self._advance()
        value: list[str] = []
        terminated = False
        while not self._at_end:
            if triple and self._source.startswith(delimiter, self._offset):
                for _ in delimiter:
                    self._advance()
                terminated = True
                break
            character = self._advance()
            if not triple and character == quote:
                terminated = True
                break
            if not triple and character in "\r\n":
                break
            if character == "\\" and not self._at_end:
                value.append(self._decode_escape(self._advance()))
            else:
                value.append(character)
        span = SourceSpan(start, self._position)
        raw = self._source[start.offset : self._offset]
        if not terminated:
            self._diagnostics.append(
                SugarScriptDiagnostic(
                    "sugarscript.lex.unterminated_string",
                    "Unterminated string literal.",
                    DiagnosticSeverity.ERROR,
                    span,
                )
            )
        return Token(TokenKind.STRING, "".join(value), raw, span)

    def _read_number(self) -> Token:
        """Read one unsigned integer or decimal token."""

        start = self._position
        while self._peek().isdigit():
            self._advance()
        if self._peek() == "." and self._peek(1).isdigit():
            self._advance()
            while self._peek().isdigit():
                self._advance()
        raw = self._source[start.offset : self._offset]
        return Token(TokenKind.NUMBER, raw, raw, SourceSpan(start, self._position))

    def _read_identifier(self) -> Token:
        """Read a Unicode identifier; keyword interpretation belongs to the parser."""

        start = self._position
        while self._peek().isalnum() or self._peek() == "_":
            self._advance()
        raw = self._source[start.offset : self._offset]
        return Token(TokenKind.IDENTIFIER, raw, raw, SourceSpan(start, self._position))

    def _read_symbol(self) -> Token:
        """Read one known punctuation token."""

        start = self._position
        raw = self._advance()
        return Token(_SYMBOLS[raw], raw, raw, SourceSpan(start, self._position))

    @staticmethod
    def _decode_escape(character: str) -> str:
        """Decode the stable escape subset supported by legacy SugarScript."""

        decoded = {
            "n": "\n",
            "r": "\r",
            "t": "\t",
            "\\": "\\",
            '"': '"',
            "'": "'",
        }.get(character)
        return decoded if decoded is not None else f"\\{character}"
