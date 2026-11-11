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
"""Adapt registered external originals to Comfy's native mask upload without replacing its controls."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
import json
import logging
from pathlib import Path
import shutil
import uuid

from aiohttp import web
from multidict import MultiDictProxy

from .artifact_registry import MANAGED_SUBFOLDER_PREFIX, ManagedArtifactRegistry

_LOGGER = logging.getLogger(__name__)


class NativeManagedMaskBridge:
    """Stage only a registered original while leaving native mask validation and output writing authoritative."""

    def __init__(self, registry: ManagedArtifactRegistry, temporary_root: Path) -> None:
        """Bind approved file identities and the current host's temporary directory."""
        self._registry = registry
        self._root = temporary_root.resolve()
        self._slots = asyncio.Semaphore(2)

    async def response(
        self,
        request: web.Request,
        handler: Callable[[web.Request], Awaitable[web.StreamResponse]],
    ) -> web.StreamResponse:
        """Translate only a managed original reference and remove its staging file on every outcome."""
        fields = await request.post()
        raw = fields.get("original_ref")
        if not isinstance(raw, str):
            return await handler(request)
        try:
            original: object = json.loads(raw)
        except ValueError:
            return await handler(request)
        if not isinstance(original, dict):
            return await handler(request)
        subfolder = original.get("subfolder")
        if not isinstance(subfolder, str) or not subfolder.startswith(
            MANAGED_SUBFOLDER_PREFIX
        ):
            return await handler(request)
        filename, output_type = original.get("filename"), original.get("type", "output")
        if not isinstance(filename, str) or not isinstance(output_type, str):
            raise web.HTTPBadRequest(text="The managed mask original is invalid.")
        source = self._registry.resolve(
            subfolder[len(MANAGED_SUBFOLDER_PREFIX) :], filename, output_type
        )
        if source is None:
            raise web.HTTPNotFound(text="The managed mask original is unavailable.")
        async with self._slots:
            staged = (
                self._root
                / "SugarCubes_mask_sources"
                / f"{uuid.uuid4()}{source.suffix}"
            )
            try:
                await _settled_thread(self._stage, source, staged)
                adapted = fields.copy()
                adapted["original_ref"] = json.dumps(
                    {
                        "filename": staged.name,
                        "subfolder": "SugarCubes_mask_sources",
                        "type": "temp",
                    }
                )
                # aiohttp caches parsed multipart data here; Comfy's closure reads
                # request.post() again. Preserve all upload fields and file handles.
                request._post = MultiDictProxy(adapted)
                return await handler(request)
            finally:
                request._post = fields
                await _settled_thread(self._remove, staged)

    def _stage(self, source: Path, destination: Path) -> None:
        """Copy bytes without decoding, preserving canonical PNG workflow metadata for the native editor."""
        destination.parent.mkdir(parents=True, exist_ok=True)
        if not destination.parent.resolve().is_relative_to(self._root):
            raise ValueError(
                "Managed mask staging escaped the host temporary directory."
            )
        with destination.open("xb") as target, source.open("rb") as original:
            shutil.copyfileobj(original, target, length=1024 * 1024)

    def _remove(self, destination: Path) -> None:
        """Delete only this operation's contained temporary original after native upload completes."""
        if not destination.resolve().is_relative_to(self._root):
            raise ValueError(
                "Managed mask cleanup escaped the host temporary directory."
            )
        try:
            destination.unlink(missing_ok=True)
        except OSError:
            _LOGGER.exception(
                "Native mask upload completed but its temporary managed original could not be removed."
            )


async def _settled_thread(
    function: Callable[..., object], *args: object, **kwargs: object
) -> None:
    """Wait for an owned filesystem operation before cleanup or releasing its bounded concurrency slot."""
    task = asyncio.create_task(asyncio.to_thread(function, *args, **kwargs))
    try:
        await asyncio.shield(task)
    except asyncio.CancelledError:
        await task
        raise
