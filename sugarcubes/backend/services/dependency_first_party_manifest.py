#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Declare trusted first-party custom-node acquisition identities."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .dependency_requirements import normalize_requirement_key


@dataclass(frozen=True, slots=True)
class FirstPartyExtension:
    """Describe one extension SugarCubes may acquire outside the Registry."""

    registry_id: str
    project_name: str
    repository_url: str
    target_folder_name: str
    sentinel_files: tuple[Path, ...]
    tag_prefix: str = "v"
    requirements_file: Path | None = Path("requirements.txt")

    def archive_url(self, version: str) -> str:
        """Return the trusted GitHub archive URL for one exact release tag."""

        repository = self.repository_url.removesuffix(".git").rstrip("/")
        return f"{repository}/archive/refs/tags/{self.tag_prefix}{version}.zip"


_FIRST_PARTY_EXTENSIONS = (
    FirstPartyExtension(
        registry_id="SimpleSyrup",
        project_name="SimpleSyrup",
        repository_url="https://github.com/Artificial-Sweetener/SimpleSyrup.git",
        target_folder_name="SimpleSyrup",
        sentinel_files=(
            Path("__init__.py"),
            Path("pyproject.toml"),
            Path("simple_syrup") / "__init__.py",
        ),
    ),
)

FIRST_PARTY_EXTENSIONS = {
    normalize_requirement_key(extension.registry_id): extension
    for extension in _FIRST_PARTY_EXTENSIONS
}


def first_party_extension(node_id: str) -> FirstPartyExtension | None:
    """Return trusted acquisition metadata for one Registry identity."""

    return FIRST_PARTY_EXTENSIONS.get(normalize_requirement_key(node_id))


__all__ = [
    "FIRST_PARTY_EXTENSIONS",
    "FirstPartyExtension",
    "first_party_extension",
]
