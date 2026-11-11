#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Emit bounded console progress for dependency version repair."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from typing import Any

from .cube_metadata import normalize_metadata_string

_logger = logging.getLogger(__name__)


def log_version_selected(item: Mapping[str, Any]) -> None:
    """Report the authoritative version selected from Cube requirements."""

    node_id, installed, required = _version_facts(item)
    _logger.info(
        "SugarCubes[nodepack_version_selected]: %s %s is the highest "
        "cube-required version; installed version is %s.",
        node_id,
        required,
        installed,
    )


def log_update_started(item: Mapping[str, Any]) -> None:
    """Report the start of one approved dependency update."""

    node_id, installed, required = _version_facts(item)
    _logger.info(
        "SugarCubes[nodepack_update_started]: Updating %s from %s to %s.",
        node_id,
        installed,
        required,
    )


def log_source_selected(item: Mapping[str, Any], source: object) -> None:
    """Report the approved acquisition source for one dependency update."""

    node_id, _installed, required = _version_facts(item)
    selected_source = normalize_metadata_string(source) or "shared acquisition policy"
    _logger.info(
        "SugarCubes[nodepack_source_selected]: %s %s will use %s.",
        node_id,
        required,
        selected_source,
    )


def log_update_complete(item: Mapping[str, Any]) -> None:
    """Report completion of one approved dependency update."""

    node_id, installed, required = _version_facts(item)
    _logger.info(
        "SugarCubes[nodepack_update_complete]: Updated %s from %s to %s.",
        node_id,
        installed,
        required,
    )


def log_restart_required() -> None:
    """Report that completed dependency changes require a ComfyUI restart."""

    _logger.info(
        "SugarCubes[nodepack_restart_required]: Dependency updates are complete; "
        "restart ComfyUI to load them."
    )


def log_update_failed(item: Mapping[str, Any], reason: object) -> None:
    """Report one actionable non-fatal dependency update failure."""

    node_id, installed, required = _version_facts(item)
    failure_reason = (
        normalize_metadata_string(reason).rstrip(".") or "unknown repair failure"
    )
    _logger.warning(
        "SugarCubes[nodepack_update_failed]: %s could not be updated from %s "
        "to required %s: %s. ComfyUI will continue starting.",
        node_id,
        installed,
        required,
        failure_reason,
    )


def _version_facts(item: Mapping[str, Any]) -> tuple[str, str, str]:
    """Return printable node, installed-version, and required-version facts."""

    return (
        normalize_metadata_string(item.get("nodeId")) or "A node pack",
        normalize_metadata_string(item.get("installedVersion")) or "unknown",
        normalize_metadata_string(item.get("requiredVersion")) or "unknown",
    )
