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
"""Prove desktop acquisition retains exact current provider identity and host-role safety."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, replace

import pytest

from sugarcubes.model_library.application.requested_artifacts import (
    RequestedModelArtifacts,
)
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from tests.model_library.support.model_releases import release


@dataclass(frozen=True, slots=True)
class RecipeProvider:
    """Replace external hash/history lookup while preserving typed provider evidence."""

    identified: ProviderModelVersion
    history: tuple[ModelProviderRelease, ...]

    async def requested_version(self, sha256: str) -> ProviderModelVersion | None:
        """Return verified hash identity without reading weights."""
        return self.identified

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return freshly fetched history that may contradict an earlier hash response."""
        return self.history


@pytest.mark.parametrize("contradiction", ["parent", "version", "provider", "type"])
def test_current_history_cannot_authorize_a_stale_or_foreign_hash_lookup(
    contradiction: str,
) -> None:
    """Reject a matching SHA-256 when the current release no longer belongs to the identified version."""
    valid = release("20")
    changed = (
        replace(valid.version, model_id="999")
        if contradiction == "parent"
        else (
            replace(valid.version, version_id="21")
            if contradiction == "version"
            else (
                replace(valid.version, provider="openmodeldb")
                if contradiction == "provider"
                else replace(valid.version, model_type="Checkpoint")
            )
        )
    )
    subject = RequestedModelArtifacts(
        RecipeProvider(valid.version, (replace(valid, version=changed),))
    )
    with pytest.raises(ValueError, match="verified"):
        asyncio.run(subject.offer("a" * 64, "loras"))


def test_diffusion_recipe_uses_provider_checkpoint_type_and_retains_host_destination() -> (
    None
):
    """Share the family policy's host mapping without changing file or content identities."""
    valid = release("20", kind="checkpoints")
    subject = RequestedModelArtifacts(RecipeProvider(valid.version, (valid,)))
    offer = asyncio.run(subject.offer("a" * 64, "diffusion_models"))
    assert offer.kind == "diffusion_models"
    assert offer.file_id == valid.offers[0].file_id
    assert offer.sha256 == valid.offers[0].sha256
    with pytest.raises(ValueError, match="verified"):
        asyncio.run(subject.offer("a" * 64, "loras"))
