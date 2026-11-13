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
"""HTTP response helpers and typed backend errors for SugarCubes."""

from __future__ import annotations

from typing import Any, Mapping, MutableMapping, Optional

from aiohttp import web


class BackendError(RuntimeError):
    """Represent an expected backend failure with an HTTP status."""

    def __init__(
        self,
        message: str,
        *,
        status: int,
        details: Any = None,
        extra: Optional[Mapping[str, Any]] = None,
    ) -> None:
        """Initialize the backend error payload.

        Args:
            message: User-facing error message.
            status: HTTP status code to return.
            details: Optional structured details payload.
            extra: Optional extra keys to merge into the error body.
        """

        super().__init__(message)
        self.message = message
        self.status = status
        self.details = details
        self.extra = dict(extra or {})


def build_error_payload(
    message: str,
    *,
    details: Any = None,
    extra: Optional[Mapping[str, Any]] = None,
) -> dict[str, Any]:
    """Build the repository-standard JSON error payload."""

    payload: MutableMapping[str, Any] = {"message": message}
    if details is not None:
        payload["details"] = details
    if extra:
        payload.update(extra)
    return {"error": dict(payload)}


def json_error(
    message: str,
    *,
    status: int,
    details: Any = None,
    extra: Optional[Mapping[str, Any]] = None,
) -> web.Response:
    """Return a JSON error response in the repository-standard shape."""

    return web.json_response(
        build_error_payload(message, details=details, extra=extra),
        status=status,
    )


def json_error_from_exception(error: BackendError) -> web.Response:
    """Return a JSON error response from a `BackendError`."""

    return json_error(
        error.message,
        status=error.status,
        details=error.details,
        extra=error.extra,
    )


def json_success(payload: Mapping[str, Any], *, status: int = 200) -> web.Response:
    """Return a successful JSON response."""

    return web.json_response(dict(payload), status=status)
