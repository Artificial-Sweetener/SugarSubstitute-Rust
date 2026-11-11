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
"""Own provider lineage and directional addon compatibility independently of hosted generation."""

from __future__ import annotations

from dataclasses import dataclass, field
from collections.abc import Mapping
from types import MappingProxyType
from typing import Literal

from .architecture import ModelArchitecture
from .models import ModelMetadata

Compatibility = Literal["full", "partial", "incompatible", "unknown"]


@dataclass(frozen=True, slots=True)
class ModelFamily:
    """Describe provider identity separately from the installed tensor architecture."""

    key: str
    name: str
    parent: str
    architecture: str
    bases: tuple[str, ...]
    model_types: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class CompatibilityRule:
    """Preserve the direction and applicable resource types of an explicit provider rule."""

    source: str
    target: str
    model_types: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class ModelRelationships:
    """Share one immutable relationship policy among every catalog consumer."""

    revision: str
    families: tuple[ModelFamily, ...]
    rules: tuple[CompatibilityRule, ...]
    _labels: Mapping[str, ModelFamily] = field(init=False, repr=False, compare=False)
    _targets: Mapping[tuple[str, str], tuple[tuple[str, str], ...]] = field(
        init=False, repr=False, compare=False
    )

    def __post_init__(self) -> None:
        """Prepare immutable indexes once so catalog refresh does not repeat relationship scans."""
        labels: dict[str, ModelFamily] = {}
        for family in self.families:
            for label in (family.key, family.name, *family.bases):
                labels.setdefault(label.casefold(), family)
        object.__setattr__(self, "_labels", MappingProxyType(labels))
        types = {"", "Checkpoint", "LORA", "LoCon", "TextualInversion", "VAE"}
        types.update(kind for rule in self.rules for kind in rule.model_types)
        targets = {
            (source.key, kind): tuple(
                (target.key, support)
                for target in self.families
                for support in (self.compatibility(target.key, source.key, kind),)
                if support in ("full", "partial")
            )
            for source in self.families
            for kind in types
        }
        object.__setattr__(self, "_targets", MappingProxyType(targets))

    def resolve(self, label: str) -> ModelFamily | None:
        """Match exact declared names and stable keys without guessing from model filenames."""
        return self._labels.get(label.strip().casefold())

    def compatibility(self, target: str, source: str, model_type: str) -> Compatibility:
        """Evaluate addon-to-checkpoint direction without treating ancestry as compatibility."""
        target_family, source_family = self.resolve(target), self.resolve(source)
        if target_family is None or source_family is None:
            return "unknown"
        if target_family.key == source_family.key:
            return "full" if model_type in target_family.model_types else "incompatible"
        root = self.root(target_family)
        if any(
            rule.source == source_family.key
            and rule.target in (target_family.key, root.key)
            and (not rule.model_types or model_type in rule.model_types)
            for rule in self.rules
        ):
            return "partial"
        return "incompatible"

    def root(self, family: ModelFamily) -> ModelFamily:
        """Follow validated identity ancestry without attributing compatibility to that chain."""
        seen: set[str] = set()
        while family.parent:
            if family.key in seen:
                raise ValueError("Cyclic model family ancestry.")
            seen.add(family.key)
            parent = self.resolve(family.parent)
            if parent is None:
                raise ValueError("Unknown model family parent.")
            family = parent
        return family

    def traits(
        self,
        metadata: ModelMetadata | None,
        architecture: ModelArchitecture | None,
        kind: str,
        nsfw_override: bool | None = None,
    ) -> dict[str, object]:
        """Publish reusable evidence and decisions while retaining unknown and conflicting facts."""
        family = self.resolve(metadata.base_model) if metadata else None
        inferred = family.architecture if family else ""
        detected = architecture.base if architecture else ""
        conflict = bool(detected and inferred and detected != inferred)
        model_type = (
            metadata.model_type
            if metadata and metadata.model_type
            else {
                "loras": "LORA",
                "checkpoints": "Checkpoint",
                "diffusion_models": "Checkpoint",
                "embeddings": "TextualInversion",
                "vae": "VAE",
            }.get(kind, "")
        )
        provider_nsfw = metadata.provider_nsfw if metadata else None
        nsfw = nsfw_override if nsfw_override is not None else provider_nsfw
        return {
            "architecture": detected or inferred,
            "architectureSource": (
                "comfy_tensor_shapes"
                if architecture
                else "civitai_relationship" if inferred else "unknown"
            ),
            "family": family.key if family else "",
            "familyName": family.name if family else "",
            "familySource": (
                "civitai"
                if family and metadata and metadata.provider == "civitai"
                else "metadata" if family else "unknown"
            ),
            "modelType": model_type,
            "conflict": conflict,
            "relationshipRevision": self.revision,
            "nsfw": "nsfw" if nsfw is True else "sfw" if nsfw is False else "unknown",
            "providerNsfw": provider_nsfw,
            "providerNsfwLevel": metadata.provider_nsfw_level if metadata else None,
            "nsfwOverride": nsfw_override,
            "compatibleTargets": [
                {"family": target, "support": support}
                for target, support in (
                    self._targets.get(
                        (family.key, model_type), self._targets[(family.key, "")]
                    )
                    if family and not conflict
                    else ()
                )
            ],
        }

    def to_payload(self) -> dict[str, object]:
        """Expose portable family assignments and all explicit directional rules to clients."""
        return {
            "schemaVersion": 1,
            "revision": self.revision,
            "families": [
                {
                    "key": family.key,
                    "name": family.name,
                    "parent": family.parent,
                    "architecture": family.architecture,
                    "bases": list(family.bases),
                }
                for family in self.families
            ],
            "rules": [
                {
                    "source": rule.source,
                    "target": rule.target,
                    "modelTypes": list(rule.model_types),
                    "support": "partial",
                }
                for rule in self.rules
            ],
        }
