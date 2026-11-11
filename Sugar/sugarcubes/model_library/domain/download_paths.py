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
"""Render portable user download patterns without allowing path-bearing token values."""

from __future__ import annotations

from collections.abc import Mapping
import re

_TOKENS = frozenset(
    {
        "base_model",
        "model_name",
        "version_name",
        "creator",
        "file_name",
        "file_stem",
        "architecture",
        "family",
        "nsfw",
    }
)
_TOKEN = re.compile(r"\{([^{}]+)\}")
_SEPARATORS = re.compile(r"[\\/]+")


def download_relative_path(
    file_name: str, pattern: str, tokens: Mapping[str, str]
) -> str:
    """Preserve desktop path conventions while rejecting absolute paths and traversal."""
    selected = pattern.strip() or "{file_name}"
    if (
        len(selected) > 2048
        or re.match(r"^[A-Za-z]:", selected)
        or selected.startswith(("/", "\\"))
    ):
        raise ValueError("The model download path pattern must be relative.")
    remainder = _TOKEN.sub("", selected)
    if "{" in remainder or "}" in remainder:
        raise ValueError("The model download path pattern has malformed token syntax.")
    unknown = set(_TOKEN.findall(selected)) - _TOKENS
    if unknown:
        raise ValueError(
            "Unknown model download path token(s): " + ", ".join(sorted(unknown))
        )
    components = tuple(
        part for part in _SEPARATORS.split(selected.rstrip("/\\")) if part
    )
    final_tokens = set(_TOKEN.findall(components[-1])) if components else set()
    if (
        selected.endswith(("/", "\\"))
        or final_tokens
        and final_tokens <= {"architecture", "family", "nsfw"}
    ):
        components = (*components, "{file_name}")
    if not components or any(part in (".", "..") for part in components):
        raise ValueError("The model download path pattern cannot contain traversal.")
    source_name = (
        tokens.get("fileName", file_name).replace("\\", "/").rsplit("/", 1)[-1]
    )
    name = _component(source_name)
    if not _extension(name):
        name += ".safetensors"
    values = {
        "base_model": _base_model(tokens.get("baseModel", "")) or "Unsorted",
        "model_name": tokens.get("modelName", "").strip() or "Unsorted",
        "version_name": tokens.get("versionName", "").strip() or "Version",
        "creator": tokens.get("creator", "").strip() or "Unknown Creator",
        "file_name": name,
        "file_stem": name.rsplit(".", 1)[0],
        "architecture": tokens.get("architecture", "").strip() or "Unknown",
        "family": tokens.get("family", "").strip() or "Unknown",
        "nsfw": (
            tokens.get("nsfw", "unknown")
            if tokens.get("nsfw") in ("sfw", "nsfw")
            else "unknown"
        ),
    }
    result: list[str] = []
    for index, part in enumerate(components):
        rendered = _component(
            _TOKEN.sub(lambda match: _component(values[match.group(1)]), part)
        )
        if index == len(components) - 1 and not _extension(rendered):
            rendered += _extension(name)
        result.append(rendered)
    return "/".join(result)


def valid_download_relative_path(value: str) -> bool:
    """Accept only normalized portable components beneath an approved installation root."""
    if (
        len(value) > 2048
        or "\\" in value
        or value.startswith("/")
        or re.match(r"^[A-Za-z]:", value)
    ):
        return False
    try:
        return all(
            part not in ("", ".", "..") and _component(part) == part
            for part in value.split("/")
        )
    except ValueError:
        return False


def _component(value: str) -> str:
    """Sanitize literal labels without letting one token introduce a directory."""
    cleaned = re.sub(r'[<>:"/\\|?*\x00-\x1f]', "_", value.strip())
    cleaned = re.sub(r"\s+", " ", cleaned)
    cleaned = re.sub(r"_+", "_", cleaned).strip(" ._")
    reserved = {
        "CON",
        "PRN",
        "AUX",
        "NUL",
        *(f"COM{i}" for i in range(1, 10)),
        *(f"LPT{i}" for i in range(1, 10)),
    }
    if not cleaned or len(cleaned) > 200 or cleaned.split(".")[0].upper() in reserved:
        raise ValueError("The model download path contains an invalid component.")
    return cleaned


def _base_model(value: str) -> str:
    """Retain the established desktop buckets used by persisted path patterns."""
    folded = value.strip().casefold()
    for marker, bucket in (
        ("anima", "Anima"),
        ("illustrious", "Illustrious"),
        ("pony", "Pony"),
        ("flux", "Flux"),
        ("sdxl", "SDXL"),
        ("stable diffusion xl", "SDXL"),
    ):
        if marker in folded:
            return bucket
    if folded in ("stable diffusion 1.5", "sd 1.5", "sd1.5", "sd15"):
        return "SD 1.5"
    if "wan" in folded:
        for version in ("2.2", "2.1"):
            if version in folded:
                return "WAN " + version
    return value.strip()


def _extension(name: str) -> str:
    """Extract a portable filename suffix without importing filesystem APIs."""
    return "." + name.rsplit(".", 1)[-1] if "." in name else ""
