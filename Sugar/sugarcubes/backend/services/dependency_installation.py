#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Execute approved custom-node installation work."""

from __future__ import annotations

import logging
from collections.abc import Mapping, Sequence
from typing import Any

from .cube_metadata import normalize_metadata_string
from .dependency_acquisition import DependencyAcquirer

_logger = logging.getLogger(__name__)


class DependencyNodeInstaller:
    """Install selected dependency plan items through acquisition policy."""

    def __init__(self, *, acquirer: DependencyAcquirer) -> None:
        """Initialize the installer with explicit host boundaries."""

        self._acquirer = acquirer

    def install(
        self,
        selected_items: Sequence[Mapping[str, Any]],
    ) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
        """Return attempted items, successful installs, and failures."""

        attempted = [dict(item) for item in selected_items]
        installed: list[dict[str, Any]] = []
        failed: list[dict[str, Any]] = []
        for item in attempted:
            node_id = normalize_metadata_string(item.get("nodeId"))
            try:
                install_payload = self._acquirer.acquire(item)
            except (OSError, RuntimeError, ValueError) as exc:
                _logger.exception(
                    "SugarCubes: dependency acquisition failed for node %s", node_id
                )
                failed.append(
                    {
                        "nodeId": node_id,
                        "reason": str(exc),
                        "stdout": "",
                        "stderr": "",
                    }
                )
                continue
            if install_payload.get("returnCode") == 0:
                installed.append(install_payload)
            else:
                failed.append(install_payload)
        return attempted, installed, failed
