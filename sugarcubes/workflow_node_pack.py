#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Own SugarCubes' Comfy Registry identity on serialized workflow nodes."""

from __future__ import annotations

from dataclasses import dataclass

from .package_identity import SUGARCUBES_DISTRIBUTION_NAME, runtime_version


@dataclass(frozen=True)
class WorkflowNodePackMetadata:
    """Describe the custom-node package required by a persisted workflow node."""

    cnr_id: str
    version: str

    def node_properties(self) -> dict[str, str]:
        """Project metadata into Comfy's workflow-node property contract."""

        return {"cnr_id": self.cnr_id, "ver": self.version}

    def api_payload(self) -> dict[str, str]:
        """Project metadata into SugarCubes' frontend API contract."""

        return {"cnrId": self.cnr_id, "version": self.version}


def current_workflow_node_pack() -> WorkflowNodePackMetadata:
    """Return the canonical Registry identity for this SugarCubes build."""

    return WorkflowNodePackMetadata(
        cnr_id=SUGARCUBES_DISTRIBUTION_NAME,
        version=runtime_version(),
    )


__all__ = ["WorkflowNodePackMetadata", "current_workflow_node_pack"]
