#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Resolve authoritative custom-node source identities from Comfy Registry."""

from __future__ import annotations

import json
import re
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .cube_metadata import normalize_metadata_string
from .dependency_requirements import normalize_requirement_key
from .dependency_versions import classify_version

_REGISTRY_API_ROOT = "https://api.comfy.org"
_REGISTRY_TIMEOUT_SECONDS = 20
_MAX_METADATA_BYTES = 1024 * 1024
_SAFE_NODE_ID = re.compile(r"^[A-Za-z0-9._-]+$")
RegistryNodeLoader = Callable[[str], Mapping[str, Any]]
RegistryInstallLoader = Callable[[str, str], Mapping[str, Any] | None]


@dataclass(frozen=True, slots=True)
class RegistrySource:
    """Describe one Registry-owned custom-node source repository."""

    node_id: str
    project_name: str
    repository_url: str
    target_folder_name: str
    package_url: str = ""
    package_version: str = ""
    requirements_file: Path = Path("requirements.txt")

    def archive_urls(self, required_version: str) -> tuple[str, ...]:
        """Return safe GitHub archives in exact-to-current preference order."""

        repository = self.repository_url.removesuffix(".git").rstrip("/")
        version_kind = classify_version(required_version)
        if version_kind == "semver":
            return (
                f"{repository}/archive/refs/tags/v{required_version}.zip",
                f"{repository}/archive/refs/tags/{required_version}.zip",
                f"{repository}/archive/HEAD.zip",
            )
        if version_kind == "git_sha":
            return (f"{repository}/archive/{required_version}.zip",)
        return (f"{repository}/archive/HEAD.zip",)


class RegistrySourceResolver:
    """Resolve and validate repository identity supplied by Comfy Registry."""

    def __init__(
        self,
        *,
        loader: RegistryNodeLoader | None = None,
        install_loader: RegistryInstallLoader | None = None,
    ) -> None:
        """Initialize the resolver around an injectable metadata boundary."""

        self._loader = loader or load_registry_node
        self._install_loader = install_loader or load_registry_install

    def resolve(self, node_id: str, required_version: str = "") -> RegistrySource:
        """Return a validated source identity for one exact Registry node id."""

        normalized_node_id = normalize_metadata_string(node_id)
        if not normalized_node_id or not _SAFE_NODE_ID.fullmatch(normalized_node_id):
            raise ValueError("Registry node id is not safe for installation.")
        payload = self._loader(normalized_node_id)
        observed_id = normalize_metadata_string(payload.get("id"))
        if normalize_requirement_key(observed_id) != normalize_requirement_key(
            normalized_node_id
        ):
            raise ValueError(
                "Registry source identity does not match the requested node."
            )
        repository_url = _validated_github_repository(
            normalize_metadata_string(payload.get("repository"))
        )
        install_payload = self._install_loader(normalized_node_id, required_version)
        package_url = ""
        package_version = ""
        if install_payload is not None:
            observed_install_id = normalize_metadata_string(
                install_payload.get("node_id")
            )
            if normalize_requirement_key(
                observed_install_id
            ) != normalize_requirement_key(normalized_node_id):
                raise ValueError(
                    "Registry install identity does not match the requested node."
                )
            package_url = _validated_registry_package_url(
                normalize_metadata_string(install_payload.get("downloadUrl"))
            )
            package_version = normalize_metadata_string(install_payload.get("version"))
            if (
                classify_version(required_version) == "semver"
                and package_version != required_version
            ):
                raise ValueError("Registry install version does not match the request.")
        return RegistrySource(
            node_id=observed_id,
            project_name=observed_id,
            repository_url=repository_url,
            target_folder_name=observed_id,
            package_url=package_url,
            package_version=package_version,
        )


def load_registry_node(node_id: str) -> Mapping[str, Any]:
    """Load bounded node metadata from the public Comfy Registry API."""

    encoded_node_id = urllib.parse.quote(node_id, safe="")
    request = urllib.request.Request(
        f"{_REGISTRY_API_ROOT}/nodes/{encoded_node_id}",
        headers={"Accept": "application/json", "User-Agent": "SugarCubes"},
    )
    with urllib.request.urlopen(  # noqa: S310 - fixed HTTPS Registry origin.
        request,
        timeout=_REGISTRY_TIMEOUT_SECONDS,
    ) as response:
        raw_payload = response.read(_MAX_METADATA_BYTES + 1)
    if len(raw_payload) > _MAX_METADATA_BYTES:
        raise RuntimeError("Comfy Registry node metadata exceeds the size limit.")
    try:
        payload = json.loads(raw_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError("Comfy Registry returned invalid node metadata.") from exc
    if not isinstance(payload, Mapping):
        raise RuntimeError("Comfy Registry returned invalid node metadata.")
    return payload


def load_registry_install(
    node_id: str, required_version: str
) -> Mapping[str, Any] | None:
    """Load an exact Registry package descriptor when one is published."""

    encoded_node_id = urllib.parse.quote(node_id, safe="")
    query = ""
    if classify_version(required_version) == "semver":
        query = "?" + urllib.parse.urlencode({"version": required_version})
    elif required_version:
        return None
    request = urllib.request.Request(
        f"{_REGISTRY_API_ROOT}/nodes/{encoded_node_id}/install{query}",
        headers={"Accept": "application/json", "User-Agent": "SugarCubes"},
    )
    try:
        with urllib.request.urlopen(  # noqa: S310 - fixed HTTPS Registry origin.
            request,
            timeout=_REGISTRY_TIMEOUT_SECONDS,
        ) as response:
            raw_payload = response.read(_MAX_METADATA_BYTES + 1)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return None
        raise
    if len(raw_payload) > _MAX_METADATA_BYTES:
        raise RuntimeError("Comfy Registry install metadata exceeds the size limit.")
    try:
        payload = json.loads(raw_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError("Comfy Registry returned invalid install metadata.") from exc
    if not isinstance(payload, Mapping):
        raise RuntimeError("Comfy Registry returned invalid install metadata.")
    return payload


def _validated_github_repository(value: str) -> str:
    """Return one canonical public GitHub repository URL or reject it."""

    parsed = urllib.parse.urlsplit(value)
    path_parts = tuple(part for part in parsed.path.split("/") if part)
    if (
        parsed.scheme != "https"
        or parsed.hostname != "github.com"
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port is not None
        or parsed.query
        or parsed.fragment
        or len(path_parts) != 2
    ):
        raise ValueError(
            "Registry repository must be a public HTTPS GitHub repository."
        )
    owner, repository = path_parts
    repository = repository.removesuffix(".git")
    if not owner or not repository:
        raise ValueError("Registry repository identity is incomplete.")
    return f"https://github.com/{owner}/{repository}"


def _validated_registry_package_url(value: str) -> str:
    """Return one canonical Comfy Registry package URL or reject it."""

    parsed = urllib.parse.urlsplit(value)
    if (
        parsed.scheme != "https"
        or parsed.hostname != "cdn.comfy.org"
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port is not None
        or parsed.query
        or parsed.fragment
        or not parsed.path.endswith("/node.zip")
    ):
        raise ValueError("Registry package URL is not a canonical Comfy CDN archive.")
    return value


__all__ = [
    "RegistrySource",
    "RegistrySourceResolver",
    "load_registry_install",
    "load_registry_node",
]
