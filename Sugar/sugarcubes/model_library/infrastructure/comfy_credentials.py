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
"""Resolve secure credential storage from Comfy's private system-user namespace."""

from __future__ import annotations

import importlib
from pathlib import Path

from .civitai_credential_store import WindowsCivitaiCredentialStore


def host_civitai_credential_store() -> WindowsCivitaiCredentialStore:
    """Use the host directory excluded from its public user-file routes."""
    module = importlib.import_module("folder_paths")
    getter: object = getattr(module, "get_system_user_directory", None)
    if not callable(getter):
        raise RuntimeError("Comfy's private system-user directory is unavailable.")
    directory: object = getter("sugarcubes")
    if (
        not isinstance(directory, str)
        or not directory
        or not Path(directory).is_absolute()
    ):
        raise RuntimeError("Comfy's private system-user directory is invalid.")
    return WindowsCivitaiCredentialStore(Path(directory) / "credentials")
