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
"""Inspect changed generative model headers through independent detector and cache ports."""

from __future__ import annotations

from collections.abc import Sequence
import logging
from typing import Protocol

from .ports import ModelCatalogRepository
from ..domain.architecture import DamagedModelFile, ModelArchitecture
from ..domain.models import ModelEvidence

_LOGGER = logging.getLogger(__name__)


class ModelArchitectureReader(Protocol):
    """Expose bounded host classification without leaking filesystem or tensor APIs."""

    @property
    def signature(self) -> str:
        """Identify the detector implementation for cache invalidation."""

    def inspect(self, evidence: ModelEvidence) -> ModelArchitecture | None:
        """Classify only the exact approved file observation."""


class ModelArchitectureCache(Protocol):
    """Keep detected and unknown results tied to their exact file and detector evidence."""

    def current(self, evidence: ModelEvidence, signature: str) -> bool:
        """Recognize persisted observations independently of their classification outcome."""

    def save(
        self,
        evidence: ModelEvidence,
        signature: str,
        architecture: ModelArchitecture | None,
    ) -> bool:
        """Reject stale results and publish the shared catalog revision atomically."""


class ModelArchitectureService:
    """Own refresh policy while catalog reads remain free of header IO."""

    def __init__(
        self,
        repository: ModelCatalogRepository,
        cache: ModelArchitectureCache,
        reader: ModelArchitectureReader,
    ) -> None:
        """Compose authoritative catalog observations, persistence, and host detection."""
        self._repository = repository
        self._cache = cache
        self._reader = reader

    def refresh(self, kinds: Sequence[str] | None) -> int:
        """Cache shape-only classification and isolate unreadable files from picker availability."""
        entries = tuple(
            entry
            for entry in self._repository.list_entries(kinds)
            if entry.evidence.kind in ("checkpoints", "diffusion_models", "unet")
        )
        if not entries:
            return 0
        try:
            signature = self._reader.signature
        except ImportError:
            _LOGGER.warning(
                "Comfy tensor detector is unavailable; architecture inspection will retry",
                exc_info=True,
            )
            return 0
        changed = 0
        for entry in entries:
            evidence = entry.evidence
            if self._cache.current(evidence, signature):
                continue
            try:
                architecture = self._reader.inspect(evidence)
            except ImportError:
                _LOGGER.warning(
                    "Model container inspection runtime is unavailable; inspection will retry",
                    extra={"model_id": evidence.location_id, "kind": evidence.kind},
                    exc_info=True,
                )
                continue
            except DamagedModelFile as damage:
                # A damaged user file is reported once and cached as unknown, not as a fault.
                _LOGGER.warning(
                    "Model file %s cannot be inspected for its architecture: %s",
                    evidence.relative_path,
                    damage.reason,
                    extra={
                        "model_id": evidence.location_id,
                        "kind": evidence.kind,
                        "model_path": evidence.relative_path,
                        "reason": damage.reason,
                    },
                )
                architecture = None
            except (
                OSError,
                ValueError,
                KeyError,
                IndexError,
                TypeError,
                AttributeError,
                RuntimeError,
                OverflowError,
                RecursionError,
            ):
                _LOGGER.exception(
                    "Model header architecture inspection failed",
                    extra={"model_id": evidence.location_id, "kind": evidence.kind},
                )
                architecture = None
            changed += self._cache.save(evidence, signature, architecture)
        return changed
