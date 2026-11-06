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
"""Confirm recorded artifacts through an explicit bounded file authority."""

from typing import Protocol

from ..domain.artifact import (
    ArtifactReceipt,
    ArtifactUnavailable,
    ExpectedArtifact,
    same_artifact,
)
from ..domain.transfer import OutputFileReference


class ArtifactReadBudget(Protocol):
    """Expose cancellation and finite work admission without binding an executor."""

    def check(self) -> None:
        """Reject cancellation or an exhausted finite deadline."""


class ArtifactFiles(Protocol):
    """Keep approved roots, regular handles and bounded rediscovery with the file adapter."""

    def receipt(
        self, file: OutputFileReference, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Hash and inspect one complete original through its qualified regular handle."""

    def rediscover(
        self, expected: ExpectedArtifact, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Locate only the exact recorded content within its independently approved root."""


class OutputArtifactService:
    """Resolve artifact identity without owning visible output membership or backend selection."""

    def __init__(self, files: ArtifactFiles) -> None:
        """Bind the sole server-approved filesystem authority."""
        self._files = files

    def receipt(
        self, file: OutputFileReference, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Confirm current original bytes before a checkpoint may record them."""
        budget.check()
        return self._files.receipt(file, budget)

    def resolve(
        self, expected: ExpectedArtifact, budget: ArtifactReadBudget
    ) -> ArtifactReceipt:
        """Reject overwritten files and search for missing originals only within admitted roots."""
        budget.check()
        try:
            actual = self._files.receipt(expected.receipt.file, budget)
        except ArtifactUnavailable as error:
            if error.code != "missing":
                raise
            actual = self._files.rediscover(expected, budget)
        if not same_artifact(expected.receipt, actual):
            raise ArtifactUnavailable(
                "changed", "The recorded output file has different content."
            )
        return actual
