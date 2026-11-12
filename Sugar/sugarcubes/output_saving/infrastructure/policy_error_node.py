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
"""Fail native prompt validation when required output policy preparation was rejected."""

from __future__ import annotations


class OutputSavingPolicyError:
    """Expose a prompt-only validation failure without falling back to native file writes."""

    CATEGORY = "SugarCubes/Internal"
    RETURN_TYPES: tuple[str, ...] = ()
    OUTPUT_NODE = True
    DEPRECATED = True
    FUNCTION = "fail"

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, dict[str, tuple[object, ...]]]:
        """Carry the actionable preparation failure into Comfy's existing error surface."""
        return {
            "required": {
                "message": (
                    "STRING",
                    {"default": "Output saving could not be prepared."},
                )
            }
        }

    @classmethod
    def VALIDATE_INPUTS(cls, message: str) -> str:
        """Reject every execution candidate created by the failed preparation adapter."""
        return message or "Output saving could not be prepared."

    def fail(self, message: str) -> None:
        """Keep the fallback fail-closed if a host bypasses its input validation phase."""
        raise RuntimeError(message)
