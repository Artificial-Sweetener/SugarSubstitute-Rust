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
"""Retain the codec lease through canceled requests until the background encoder releases its pixels."""

import asyncio
from threading import Event

import pytest

from sugarcubes.output_saving.application.media_service import OutputMediaService
from sugarcubes.output_saving.domain.media import (
    ImageTransferFormat,
    ImageTransferPayload,
)
from sugarcubes.output_saving.domain.preferences import OutputSavingPreferences
from sugarcubes.output_saving.domain.transfer import (
    OutputFileReference,
    OutputTransferChoices,
    TransferRepresentation,
)
from sugarcubes.output_saving.infrastructure.media_routes import _prepare


class BlockingCodec:
    """Represent only an external encoder's blocking completion boundary."""

    def __init__(self) -> None:
        """Expose explicit synchronization instead of relying on encoding speed or wall-clock sleeps."""
        self.started = Event()
        self.release = Event()
        self.calls = 0

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices:
        """Supply the external store's validated canonical image descriptor."""
        return OutputTransferChoices(TransferRepresentation(reference, "image/png"))

    @property
    def can_reveal(self) -> bool:
        """Keep desktop operations outside this codec-only fixture."""
        return False

    def reveal(self, reference: OutputFileReference) -> None:
        """Reject unrelated desktop requests at the substituted storage boundary."""
        raise ValueError("No desktop is attached to the test codec.")

    def rendition(
        self, reference: OutputFileReference, format: ImageTransferFormat, quality: int
    ) -> ImageTransferPayload:
        """Hold the first encoder's allocation until the proof deliberately releases it."""
        self.calls += 1
        if self.calls == 1:
            self.started.set()
            if not self.release.wait(10):
                raise RuntimeError("The test encoder was not released.")
        return ImageTransferPayload(b"jpeg", "image.jpg", "image/jpeg")


def test_cancellation_keeps_single_codec_lease_until_worker_finishes() -> None:
    """Prevent cancellation from allowing a second full-resolution decode to overlap the first."""

    async def exercise() -> None:
        """Exercise the asynchronous queue owner with a real worker thread and semaphore."""
        codec = BlockingCodec()
        service = OutputMediaService(OutputSavingPreferences, codec)
        semaphore = asyncio.Semaphore(1)
        reference = OutputFileReference("image.png", "", "output")
        first = asyncio.create_task(
            _prepare(semaphore, service, reference, ImageTransferFormat.JPEG)
        )
        assert await asyncio.to_thread(codec.started.wait, 5)
        first.cancel()
        await asyncio.sleep(0)
        assert semaphore.locked()
        assert not first.done()
        second = asyncio.create_task(
            _prepare(semaphore, service, reference, ImageTransferFormat.JPEG)
        )
        await asyncio.sleep(0)
        assert codec.calls == 1
        codec.release.set()
        with pytest.raises(asyncio.CancelledError):
            await first
        assert (await second).data == b"jpeg"
        assert codec.calls == 2
        assert not semaphore.locked()

    asyncio.run(exercise())
