#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Compare supported semantic versions, including numeric prereleases."""

from __future__ import annotations

SemverKey = tuple[tuple[int, int, int, int], int, tuple[tuple[int, int, str], ...]]


def semver_at_least(value: str, minimum: str) -> bool:
    """Return whether one supported semantic version meets a minimum."""

    return semver_key(value) >= semver_key(minimum)


def semver_key(value: str) -> SemverKey:
    """Return a sortable SemVer key with numeric prerelease identifiers."""

    version_without_build = value.partition("+")[0]
    main, separator, prerelease = version_without_build.partition("-")
    numeric = [int(part) for part in main.split(".") if part.isdigit()]
    padded_parts = [*numeric, 0, 0, 0, 0]
    padded = (
        padded_parts[0],
        padded_parts[1],
        padded_parts[2],
        padded_parts[3],
    )
    identifiers = tuple(
        (
            (0, int(identifier), "")
            if identifier.isdigit()
            else (1, 0, identifier.casefold())
        )
        for identifier in prerelease.split(".")
        if identifier
    )
    return (padded, 0 if separator else 1, identifiers)


__all__ = ["semver_at_least", "semver_key"]
