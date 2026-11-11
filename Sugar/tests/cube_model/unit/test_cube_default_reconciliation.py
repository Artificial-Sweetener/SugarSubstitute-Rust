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

"""Specify implementation-save default review and merge policy."""

from __future__ import annotations

from typing import Any

import pytest

from sugarcubes.cube_model import (
    CubeDocument,
    build_default_change_plan,
    merge_implementation_save_defaults,
)


def _document(
    *,
    values: dict[str, Any],
    controls: list[tuple[str, str]],
    definitions: dict[str, Any] | None = None,
) -> CubeDocument:
    """Build a cube with stable controls and embedded Comfy field definitions."""

    return CubeDocument.from_dict(
        {
            "cube_id": "local/example-user/demo.cube",
            "version": "1.0.0",
            "implementation": {
                "nodes": {},
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": definitions or {},
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": control_id,
                        "symbol": "node",
                        "input_name": input_name,
                        "label": input_name,
                        "class_type": "TestNode",
                        "value_type": (
                            "string"
                            if isinstance(values.get(control_id), str)
                            else "number"
                        ),
                    }
                    for control_id, input_name in controls
                ],
            },
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": values}]
            },
        }
    )


def _definitions(*, multiline: bool, default: str = "") -> dict[str, Any]:
    """Return one embedded string-field definition."""

    return {
        "TestNode": {
            "input": {
                "required": {
                    "prompt": [
                        "STRING",
                        {"default": default, "multiline": multiline},
                    ]
                }
            }
        }
    }


def test_changed_existing_values_require_explicit_selection() -> None:
    """Preserve existing values until their stable ids are selected."""

    existing = _document(values={"node.cfg": 7}, controls=[("node.cfg", "cfg")])
    exported = _document(values={"node.cfg": 8}, controls=[("node.cfg", "cfg")])

    plan = build_default_change_plan(existing, exported)
    preserved = merge_implementation_save_defaults(existing, exported)
    overwritten = merge_implementation_save_defaults(
        existing,
        exported,
        overwrite_control_ids=frozenset({"node.cfg"}),
        expected_fingerprint=plan.fingerprint,
    )

    assert plan.requires_review is True
    assert plan.changes[0].control_id == "node.cfg"
    assert preserved.flavors.authored[0].values == {"node.cfg": 7}
    assert overwritten.flavors.authored[0].values == {"node.cfg": 8}


def test_new_ordinary_controls_are_automatic() -> None:
    """Seed newly exposed non-text controls without interrupting the save."""

    existing = _document(values={}, controls=[])
    exported = _document(values={"node.cfg": 8}, controls=[("node.cfg", "cfg")])

    plan = build_default_change_plan(existing, exported)
    merged = merge_implementation_save_defaults(existing, exported)

    assert plan.requires_review is False
    assert plan.automatic_control_ids == ("node.cfg",)
    assert merged.flavors.authored[0].values == {"node.cfg": 8}


def test_new_multiline_control_uses_declared_default_until_selected() -> None:
    """Protect live prompt text while retaining an explicit opt-in path."""

    existing = _document(values={}, controls=[])
    exported = _document(
        values={"node.prompt": "accidental prompt"},
        controls=[("node.prompt", "prompt")],
        definitions=_definitions(multiline=True),
    )

    plan = build_default_change_plan(existing, exported)
    protected = merge_implementation_save_defaults(existing, exported)
    selected = merge_implementation_save_defaults(
        existing,
        exported,
        overwrite_control_ids=frozenset({"node.prompt"}),
        expected_fingerprint=plan.fingerprint,
    )

    assert plan.changes[0].is_multiline is True
    assert plan.changes[0].is_new is True
    assert protected.flavors.authored[0].values == {"node.prompt": ""}
    assert selected.flavors.authored[0].values == {"node.prompt": "accidental prompt"}


def test_single_line_string_control_remains_automatic() -> None:
    """Do not classify schema-confirmed single-line strings as prompt text."""

    existing = _document(values={}, controls=[])
    exported = _document(
        values={"node.prompt": "short label"},
        controls=[("node.prompt", "prompt")],
        definitions=_definitions(multiline=False),
    )

    plan = build_default_change_plan(existing, exported)

    assert plan.requires_review is False
    assert plan.automatic_control_ids == ("node.prompt",)


def test_unknown_string_control_fails_safe_as_protected_text() -> None:
    """Require opt-in when embedded schema cannot prove text is single-line."""

    existing = _document(values={}, controls=[])
    exported = _document(
        values={"node.prompt": "unclassified"},
        controls=[("node.prompt", "prompt")],
    )

    plan = build_default_change_plan(existing, exported)
    merged = merge_implementation_save_defaults(existing, exported)

    assert plan.changes[0].is_multiline is True
    assert merged.flavors.authored[0].values == {"node.prompt": ""}


def test_stale_or_unknown_decisions_are_rejected() -> None:
    """Fail closed when modal decisions do not match the recomputed plan."""

    existing = _document(values={"node.cfg": 7}, controls=[("node.cfg", "cfg")])
    exported = _document(values={"node.cfg": 8}, controls=[("node.cfg", "cfg")])

    with pytest.raises(ValueError, match="review was open"):
        merge_implementation_save_defaults(
            existing,
            exported,
            overwrite_control_ids=frozenset({"node.cfg"}),
            expected_fingerprint="stale",
        )
    with pytest.raises(ValueError, match="outside the current review"):
        merge_implementation_save_defaults(
            existing,
            exported,
            overwrite_control_ids=frozenset({"node.unknown"}),
        )
