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

"""Plan implementation-save default changes without mutating cube documents."""

from __future__ import annotations

import hashlib
import json
from copy import deepcopy
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .document import CubeDocument
from .picker_fields import find_input_field_spec
from .surface import SurfaceControl
from .surface_value_policy import tracked_surface_control_ids


@dataclass(frozen=True)
class DefaultControlChange:
    """Describe one current face value that requires an explicit save decision."""

    control_id: str
    label: str
    previous_value: Any
    proposed_value: Any
    is_new: bool
    is_multiline: bool

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-ready review row."""

        return {
            "control_id": self.control_id,
            "label": self.label,
            "previous_value": deepcopy(self.previous_value),
            "proposed_value": deepcopy(self.proposed_value),
            "is_new": self.is_new,
            "is_multiline": self.is_multiline,
        }


@dataclass(frozen=True)
class DefaultChangePlan:
    """Carry one fingerprinted default-reconciliation decision surface."""

    cube_id: str
    fingerprint: str
    changes: tuple[DefaultControlChange, ...]
    automatic_control_ids: tuple[str, ...]

    @property
    def requires_review(self) -> bool:
        """Return whether saving must pause for an explicit author decision."""

        return bool(self.changes)

    def to_dict(self) -> dict[str, Any]:
        """Return the host-neutral review contract."""

        return {
            "cube_id": self.cube_id,
            "fingerprint": self.fingerprint,
            "requires_review": self.requires_review,
            "automatic_control_ids": list(self.automatic_control_ids),
            "changes": [change.to_dict() for change in self.changes],
        }


def build_default_change_plan(
    existing: CubeDocument | None,
    exported: CubeDocument,
) -> DefaultChangePlan:
    """Compare exported face values with persisted authored defaults."""

    existing_values = _default_values(existing)
    proposed_values = _default_values(exported)
    existing_control_ids = (
        set(tracked_surface_control_ids(existing.surface.controls))
        if existing
        else set()
    )
    tracked_ids = tracked_surface_control_ids(exported.surface.controls)
    controls = {control.control_id: control for control in exported.surface.controls}
    changes: list[DefaultControlChange] = []
    automatic: list[str] = []

    for control_id in tracked_ids:
        control = controls[control_id]
        proposed_present = control_id in proposed_values
        if not proposed_present:
            continue
        proposed_value = proposed_values[control_id]
        protected = is_multiline_or_unknown_text_control(exported, control)
        existing_present = control_id in existing_values
        if existing_present:
            previous_value = existing_values[control_id]
            if previous_value != proposed_value:
                changes.append(
                    DefaultControlChange(
                        control_id=control_id,
                        label=control.label,
                        previous_value=deepcopy(previous_value),
                        proposed_value=deepcopy(proposed_value),
                        is_new=False,
                        is_multiline=protected,
                    )
                )
            continue

        safe_default = declared_control_default(exported, control)
        if protected:
            if safe_default != proposed_value:
                changes.append(
                    DefaultControlChange(
                        control_id=control_id,
                        label=control.label,
                        previous_value=deepcopy(safe_default),
                        proposed_value=deepcopy(proposed_value),
                        is_new=control_id not in existing_control_ids,
                        is_multiline=True,
                    )
                )
            continue
        automatic.append(control_id)

    return DefaultChangePlan(
        cube_id=exported.cube_id,
        fingerprint=_plan_fingerprint(existing, exported),
        changes=tuple(changes),
        automatic_control_ids=tuple(automatic),
    )


def is_multiline_or_unknown_text_control(
    document: CubeDocument,
    control: SurfaceControl,
) -> bool:
    """Protect multiline and unclassifiable string controls from implicit saves."""

    if control.value_type != "string":
        return False
    field_spec = _field_spec(document, control)
    if field_spec is None:
        return _has_text_evidence(control)
    field_type = _field_type(field_spec)
    if field_type not in {"STRING", "TEXT"}:
        return False
    metadata = _field_metadata(field_spec)
    return metadata is None or metadata.get("multiline") is not False


def _has_text_evidence(control: SurfaceControl) -> bool:
    """Conservatively recognize text widgets when embedded schema is unavailable."""

    class_name = control.class_type.casefold()
    input_name = control.input_name.casefold()
    return "string" in class_name or "prompt" in input_name or "text" in input_name


def declared_control_default(
    document: CubeDocument,
    control: SurfaceControl,
) -> Any:
    """Return a protected control's declared schema default or safe empty text."""

    metadata = _field_metadata(_field_spec(document, control))
    if metadata is not None and "default" in metadata:
        return deepcopy(metadata["default"])
    return "" if control.value_type == "string" else None


def _default_values(document: CubeDocument | None) -> Mapping[str, Any]:
    """Read one document's default authored flavor values."""

    if document is None:
        return {}
    for flavor in document.flavors.authored:
        if flavor.id == "default":
            return flavor.values
    return {}


def _field_spec(document: CubeDocument, control: SurfaceControl) -> Any | None:
    """Resolve one control's embedded Comfy input definition."""

    definition = document.implementation.definitions.get(control.class_type)
    if not isinstance(definition, Mapping):
        return None
    return find_input_field_spec(definition, control.input_name)


def _field_type(field_spec: Any) -> str:
    """Read one normalized Comfy field type."""

    if not isinstance(field_spec, Sequence) or isinstance(field_spec, (str, bytes)):
        return ""
    if not field_spec or not isinstance(field_spec[0], str):
        return ""
    return field_spec[0].upper()


def _field_metadata(field_spec: Any) -> Mapping[str, Any] | None:
    """Read metadata from one Comfy field definition."""

    if (
        isinstance(field_spec, Sequence)
        and not isinstance(field_spec, (str, bytes))
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return None


def _plan_fingerprint(
    existing: CubeDocument | None,
    exported: CubeDocument,
) -> str:
    """Hash both authoritative inputs so stale modal decisions fail closed."""

    payload = {
        "existing": existing.to_dict() if existing is not None else None,
        "exported": exported.to_dict(),
    }
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()
