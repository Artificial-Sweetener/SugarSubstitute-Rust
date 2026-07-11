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
"""Reject corrupt positional widget data before SugarCubes saves a cube."""

from __future__ import annotations

import pytest

from sugarcubes.exporter.widget_validation import (
    WidgetSaveValidationError,
    validate_serialized_widget_values,
)


def _definition(*fields: tuple[str, object]) -> dict[str, object]:
    """Build an object-info definition in a deterministic widget order."""

    required = {name: spec for name, spec in fields}
    return {
        "input": {"required": required},
        "input_order": {"required": list(required)},
    }


def test_validation_accepts_exact_live_widget_order_and_values() -> None:
    """A fully aligned sequence remains saveable without reinterpretation."""

    definition = _definition(
        ("steps", ["INT", {"default": 20, "min": 1, "max": 100}]),
        ("method", [["euler", "dpmpp"], {"default": "euler"}]),
    )

    validate_serialized_widget_values(
        node_id=10,
        class_type="Example.Node",
        persisted_widget_names=["steps", "method"],
        widget_values=[30, "dpmpp"],
        live_definition=definition,
    )


def test_validation_rejects_type_compatible_out_of_range_value() -> None:
    """A shifted number cannot evade validation merely because its type fits."""

    definition = _definition(
        ("white_point", ["FLOAT", {"default": 0.99, "min": 0.02, "max": 1.0}]),
    )

    with pytest.raises(WidgetSaveValidationError, match="white_point.*maximum 1.0"):
        validate_serialized_widget_values(
            node_id=10,
            class_type="Example.Node",
            persisted_widget_names=["white_point"],
            widget_values=[2048],
            live_definition=definition,
        )


def test_validation_rejects_widget_order_drift() -> None:
    """Inserted or reordered public inputs must not silently shift saved values."""

    definition = _definition(
        ("keep_only", ["INT", {"default": 0, "min": 0, "max": 10}]),
        ("black_point", ["FLOAT", {"default": 0.15, "min": 0.0, "max": 0.98}]),
    )

    with pytest.raises(WidgetSaveValidationError, match="widget order.*keep_only"):
        validate_serialized_widget_values(
            node_id=10,
            class_type="Example.Node",
            persisted_widget_names=["black_point"],
            widget_values=[0.15],
            live_definition=definition,
        )


def test_validation_rejects_invalid_picker_value() -> None:
    """Picker values must be among the choices reported by live Comfy."""

    definition = _definition(
        ("method", [["GuidedFilter", "VITMatte"], {"default": "GuidedFilter"}]),
    )

    with pytest.raises(WidgetSaveValidationError, match="method.*available choice"):
        validate_serialized_widget_values(
            node_id=10,
            class_type="Example.Node",
            persisted_widget_names=["method"],
            widget_values=[6],
            live_definition=definition,
        )


def test_validation_rejects_stale_control_companion() -> None:
    """A removed control-after-generate widget cannot shift following fields."""

    definition = _definition(
        ("seed", ["INT", {"default": 42, "min": 0, "max": 1000}]),
        ("resolution", ["INT", {"default": 1080, "min": 16, "max": 4096}]),
    )

    with pytest.raises(WidgetSaveValidationError, match="resolution.*integer"):
        validate_serialized_widget_values(
            node_id=10,
            class_type="Example.Node",
            persisted_widget_names=["seed", "resolution"],
            widget_values=[42, "randomize", 1080],
            live_definition=definition,
        )


def test_validation_accepts_declared_control_after_generate_companion() -> None:
    """Comfy-declared seed controls count as part of the positional contract."""

    definition = _definition(
        (
            "seed",
            [
                "INT",
                {
                    "default": 42,
                    "min": 0,
                    "max": 1000,
                    "control_after_generate": True,
                },
            ],
        ),
        ("steps", ["INT", {"default": 20, "min": 1, "max": 100}]),
    )

    validate_serialized_widget_values(
        node_id=10,
        class_type="Example.Node",
        persisted_widget_names=["seed", "steps"],
        widget_values=[42, "randomize", 30],
        live_definition=definition,
    )
