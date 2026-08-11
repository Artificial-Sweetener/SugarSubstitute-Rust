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
from pathlib import Path
from typing import Any

from ..responses import BackendError
from .cube_metadata import normalize_metadata_string
from .dependency_cli import ComfyCliAdapter

_logger = logging.getLogger(__name__)


class DependencyNodeInstaller:
    """Install selected dependency plan items through Comfy CLI."""

    def __init__(self, *, workspace_path: Path, cli_adapter: ComfyCliAdapter) -> None:
        """Initialize the installer with explicit host boundaries."""

        self._workspace_path = workspace_path
        self._cli_adapter = cli_adapter

    def install(
        self,
        selected_items: Sequence[Mapping[str, Any]],
    ) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
        """Return attempted items, successful installs, and failures."""

        attempted = [dict(item) for item in selected_items]
        installed: list[dict[str, Any]] = []
        failed: list[dict[str, Any]] = []
        if attempted:
            try:
                self._cli_adapter.assert_available(self._workspace_path)
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: Comfy CLI is unavailable for dependency repair: %s",
                    exc.message,
                )
                failed.extend(
                    _failed_install_result(item=item, error=exc) for item in attempted
                )
                return [], installed, failed

        for item in attempted:
            node_id = normalize_metadata_string(item.get("nodeId"))
            try:
                install_result = self._cli_adapter.install_node(
                    workspace_path=self._workspace_path,
                    node_id=node_id,
                )
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: dependency install failed for %s: %s",
                    node_id,
                    exc.message,
                )
                failed.append(_failed_install_result(item=item, error=exc))
                continue
            except OSError as exc:
                _logger.exception(
                    "SugarCubes: failed to launch Comfy CLI for node %s", node_id
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
            install_payload = install_result.to_payload()
            if install_result.return_code == 0:
                installed.append(install_payload)
            else:
                failed.append(
                    {
                        **install_payload,
                        "reason": "Comfy CLI failed to install the custom node",
                    }
                )
        return attempted, installed, failed


def _failed_install_result(
    *,
    item: Mapping[str, Any],
    error: BackendError,
) -> dict[str, Any]:
    """Return one failed install result from a structured backend error."""

    return {
        "nodeId": normalize_metadata_string(item.get("nodeId")),
        "reason": error.details.get("reason") or error.message,
        "stdout": normalize_metadata_string(error.details.get("stdout")),
        "stderr": normalize_metadata_string(error.details.get("stderr")),
        "status": error.status,
    }
