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
"""Persist user-encrypted CivitAI credentials atomically outside shared model assets."""

from __future__ import annotations

import base64
import logging
import os
from pathlib import Path
import sys
import tempfile
from threading import RLock

from ..application.credential_service import CredentialStorageStatus
from ..domain.credentials import validated_civitai_key
from .windows_data_protection import protect_user_data, unprotect_user_data

_LOGGER = logging.getLogger(__name__)


class WindowsCivitaiCredentialStore:
    """Use current-user DPAPI and a host-owned private directory for encrypted material."""

    def __init__(self, settings_dir: Path) -> None:
        """Bind a reviewed server path; never derive a credential destination from browser input."""
        self._path = settings_dir / "civitai_api_key.dpapi"
        self._lock = RLock()

    def status(self) -> CredentialStorageStatus:
        """Report platform availability without attempting to read credential data."""
        return CredentialStorageStatus(
            sys.platform == "win32",
            "Windows DPAPI",
            (
                None
                if sys.platform == "win32"
                else "Windows DPAPI is unavailable on this platform."
            ),
        )

    def load_api_key(self) -> str | None:
        """Fail closed for corrupt or undecryptable material without disclosing it in diagnostics."""
        with self._lock:
            if not self._path.exists():
                return None
            try:
                with self._path.open("rb") as stream:
                    content = stream.read(64 * 1024 + 1)
                if len(content) > 64 * 1024:
                    raise ValueError("Encrypted credential exceeds its bound.")
                encrypted = base64.b64decode(content, validate=True)
                return validated_civitai_key(
                    unprotect_user_data(encrypted).decode("utf-8")
                )
            except (OSError, ValueError, UnicodeDecodeError) as error:
                _LOGGER.warning(
                    "Secure CivitAI credential could not be loaded; errorType=%s",
                    type(error).__name__,
                )
                return None

    def save_api_key(self, api_key: str) -> None:
        """Encrypt before filesystem writes and publish atomically, preserving a prior key on failure."""
        key = validated_civitai_key(api_key)
        with self._lock:
            encrypted = base64.b64encode(protect_user_data(key.encode("utf-8")))
            self._path.parent.mkdir(parents=True, exist_ok=True)
            temporary: Path | None = None
            try:
                with tempfile.NamedTemporaryFile(
                    mode="wb",
                    dir=self._path.parent,
                    prefix=".civitai-key-",
                    delete=False,
                ) as stream:
                    temporary = Path(stream.name)
                    stream.write(encrypted)
                    stream.flush()
                    os.fsync(stream.fileno())
                os.replace(temporary, self._path)
            finally:
                if temporary is not None:
                    temporary.unlink(missing_ok=True)

    def clear_api_key(self) -> None:
        """Delete encrypted persisted material and propagate failed removal to the command caller."""
        with self._lock:
            self._path.unlink(missing_ok=True)


class UnavailableCivitaiCredentialStore:
    """Keep non-host composition from accidentally writing credentials to model directories."""

    def status(self) -> CredentialStorageStatus:
        """Explain that this runtime has no secure persistence adapter."""
        return CredentialStorageStatus(
            False,
            "Operating-system credential store",
            "Secure credential storage is not configured.",
        )

    def load_api_key(self) -> None:
        """Return no persisted key while allowing existing explicit host overrides."""
        return None

    def save_api_key(self, api_key: str) -> None:
        """Reject persistence instead of falling back to plaintext files."""
        raise ValueError("Secure CivitAI credential storage is unavailable.")

    def clear_api_key(self) -> None:
        """Leave absent secure storage untouched."""
