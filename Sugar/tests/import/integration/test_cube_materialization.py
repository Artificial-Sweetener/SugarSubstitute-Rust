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
"""Characterize runtime node materialization across flavor layers."""

from __future__ import annotations

from sugarcubes.cube_model.document import CubeDocument
from sugarcubes.cube_model.flavors import AuthoredFlavor, AuthoredFlavorSet
from sugarcubes.cube_model.implementation import CubeImplementation
from sugarcubes.cube_model.merge import materialize_nodes
from sugarcubes.cube_model.surface import CubeSurface, SurfaceControl


def _control(control_id: str, symbol: str, input_name: str) -> SurfaceControl:
    """Build one materialization control."""

    return SurfaceControl(
        control_id=control_id,
        symbol=symbol,
        input_name=input_name,
        label=input_name,
        class_type="KSampler",
        value_type="number",
    )


def _document() -> CubeDocument:
    """Build one document with competing authored, local, and live values."""

    return CubeDocument(
        cube_id="local/example-user/materialization.cube",
        version="1.0.0",
        implementation=CubeImplementation(
            nodes={
                "sampler": {
                    "class_type": "KSampler",
                    "inputs": {"cfg": 4.0, "steps": 20},
                },
                "invalid_inputs": {
                    "class_type": "KSampler",
                    "inputs": "legacy-value",
                },
            }
        ),
        surface=CubeSurface(
            default_flavor_id="default",
            controls=(
                _control("sampler.cfg", "sampler", "cfg"),
                _control("sampler.steps", "sampler", "steps"),
                _control("invalid.seed", "invalid_inputs", "seed"),
                _control("missing.value", "missing", "value"),
            ),
        ),
        flavors=AuthoredFlavorSet(
            authored=(
                AuthoredFlavor(
                    id="default",
                    name="Default",
                    values={
                        "sampler.cfg": 6.5,
                        "sampler.steps": 24,
                        "invalid.seed": 10,
                    },
                ),
                AuthoredFlavor(
                    id="cinematic",
                    name="Cinematic",
                    values={"sampler.cfg": 8.0, "sampler.steps": 32},
                ),
            )
        ),
    )


def test_materialization_applies_authored_local_and_live_values_in_precedence_order() -> (
    None
):
    """Live values win over local values, which win over authored defaults."""

    document = _document()

    nodes = materialize_nodes(
        document,
        local_values={"sampler.cfg": 7.0, "sampler.steps": 28},
        live_values={"sampler.cfg": 9.5},
    )

    assert nodes["sampler"]["inputs"] == {"cfg": 9.5, "steps": 28}
    assert nodes["invalid_inputs"]["inputs"] == {"seed": 10}


def test_materialization_selects_an_explicit_authored_flavor() -> None:
    """An explicit authored flavor replaces the surface default selection."""

    nodes = materialize_nodes(_document(), authored_flavor_id="cinematic")

    assert nodes["sampler"]["inputs"] == {"cfg": 8.0, "steps": 32}


def test_materialization_ignores_unknown_flavors_controls_and_missing_nodes() -> None:
    """Stale external selections and values do not create undeclared node state."""

    nodes = materialize_nodes(
        _document(),
        authored_flavor_id="removed-flavor",
        live_values={"unknown.control": 1, "missing.value": 2},
    )

    assert nodes["sampler"]["inputs"] == {"cfg": 4.0, "steps": 20}
    assert "missing" not in nodes


def test_materialization_returns_independent_node_and_value_copies() -> None:
    """Runtime mutation cannot alter the persisted implementation or flavor values."""

    document = _document()
    nested_live_value = {"schedule": [1, 2, 3]}

    nodes = materialize_nodes(
        document,
        live_values={"sampler.cfg": nested_live_value},
    )
    nodes["sampler"]["inputs"]["cfg"]["schedule"].append(4)

    assert nested_live_value == {"schedule": [1, 2, 3]}
    assert document.implementation.nodes["sampler"]["inputs"] == {
        "cfg": 4.0,
        "steps": 20,
    }
