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
"""Read server credentials without passing authentication material to browser clients."""

from __future__ import annotations

import os
from collections.abc import Callable
from collections.abc import Iterator
from contextlib import contextmanager
from contextvars import ContextVar

from ..domain.credentials import validated_civitai_key

_REQUEST_CREDENTIAL: ContextVar[str | None] = ContextVar(
    "model_request_credential", default=None
)


def civitai_api_key(stored_key: Callable[[], str | None] = lambda: None) -> str | None:
    """Resolve task, extension environment, general environment, then secure persisted material."""
    return (
        _REQUEST_CREDENTIAL.get()
        or os.environ.get("SUGARCUBES_CIVITAI_API_KEY")
        or os.environ.get("CIVITAI_API_KEY")
        or stored_key()
    )


def civitai_authorization(source: Callable[[], str | None]) -> dict[str, str]:
    """Validate credentials once for every server-owned metadata and artifact request."""
    key = source()
    if key is None or key == "":
        return {}
    return {"Authorization": f"Bearer {validated_civitai_key(key)}"}


@contextmanager
def model_request_credentials(key: str | None) -> Iterator[None]:
    """Scope an existing desktop credential to its job without persisting or leaking it to other tasks."""
    civitai_authorization(lambda: key)
    token = _REQUEST_CREDENTIAL.set(key)
    try:
        yield
    finally:
        _REQUEST_CREDENTIAL.reset(token)
