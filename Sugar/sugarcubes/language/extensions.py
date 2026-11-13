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
"""Interpret persisted SugarSubstitute comments without owning editor state."""

from __future__ import annotations

import json
import re
from collections.abc import Mapping

from .lexer import Token
from .source import DiagnosticSeverity, SugarScriptDiagnostic
from .syntax import SugarScriptComment

_JSON_EXTENSIONS = (
    "node_revealed",
    "node_enabled",
    "global_override_selection",
    "global_override_value",
    "seed_control",
    "global_override_seed_control",
    "cube_output_persistence",
    "lora_sha256",
)
_SHA256_PATTERN = re.compile(r"^sha256\s+([A-Fa-f0-9]{64})\s*$")


def decode_comment(
    token: Token,
) -> tuple[SugarScriptComment, SugarScriptDiagnostic | None]:
    """Decode a supported extension while retaining the exact original comment."""

    text = token.value.strip()
    if text.startswith("Project:"):
        return (
            SugarScriptComment(token.raw, "project", text[8:].strip(), token.span),
            None,
        )
    sha_match = _SHA256_PATTERN.fullmatch(text)
    if sha_match:
        return (
            SugarScriptComment(
                token.raw, "sha256", sha_match.group(1).lower(), token.span
            ),
            None,
        )
    for extension_name in _JSON_EXTENSIONS:
        prefix = f"{extension_name} "
        if not text.startswith(prefix):
            continue
        raw_payload = text[len(prefix) :]
        try:
            payload = json.loads(raw_payload)
        except json.JSONDecodeError:
            return _invalid_extension(
                token, extension_name, "Expected a valid JSON payload."
            )
        if not isinstance(payload, Mapping):
            return _invalid_extension(
                token, extension_name, "Expected a JSON object payload."
            )
        normalized = {str(key): value for key, value in payload.items()}
        return (
            SugarScriptComment(token.raw, extension_name, normalized, token.span),
            None,
        )
    return SugarScriptComment(token.raw, None, None, token.span), None


def _invalid_extension(
    token: Token,
    extension_name: str,
    reason: str,
) -> tuple[SugarScriptComment, SugarScriptDiagnostic]:
    """Preserve malformed extension text while reporting a non-fatal diagnostic."""

    comment = SugarScriptComment(token.raw, extension_name, None, token.span)
    diagnostic = SugarScriptDiagnostic(
        "sugarscript.extension.invalid_payload",
        f"Invalid {extension_name} metadata comment. {reason}",
        DiagnosticSeverity.WARNING,
        token.span,
    )
    return comment, diagnostic
