#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#    SPDX-License-Identifier: AGPL-3.0-or-later
"""Own explicit sibling-extension preparation before Comfy validates a Cube prompt."""

from __future__ import annotations

from typing import Protocol

from .models import ApiPrompt


class PromptPreparer(Protocol):
    """Install dependencies in the final prompt before authoritative host validation."""

    def prepare(self, prompt: ApiPrompt) -> None:
        """Prepare in place or raise to prevent queue insertion."""
