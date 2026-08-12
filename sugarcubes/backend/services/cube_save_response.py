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
"""Project finalized Cube saves into stable host-facing responses."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

from .cube_metadata import normalize_metadata_string
from .cube_save_models import FinalizedCubeSave


def project_finalized_save(finalized: FinalizedCubeSave) -> dict[str, Any]:
    """Project one authoritative persisted save into the HTTP response."""

    commit_result = finalized.commit_state.commit_result
    return {
        **finalized.artifact,
        "cube_id": finalized.target.cube_id,
        "forked": False,
        "committed": bool(commit_result),
        "commit_sha": commit_result.commit_sha if commit_result else "",
        "commit_short_sha": commit_result.commit_short_sha if commit_result else "",
        "commit_message": commit_result.commit_message if commit_result else "",
        "commit_error": finalized.commit_state.commit_error,
        "version": normalize_metadata_string(finalized.document.version),
        "definition": deepcopy(dict(finalized.definition)),
    }
