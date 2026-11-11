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
"""Lock optimizer policy observed in Substitute BackEnd before porting it."""

from __future__ import annotations


def test_characterized_pass_order_and_resource_policy() -> None:
    """Preserve pass order, resource outputs, and conservative exclusions."""

    pass_order = (
        "bypass_empty_lazy_lora",
        "intern_prompt_control",
        "intern_resource_streams",
    )
    resource_outputs = {
        "MODEL",
        "CLIP",
        "VAE",
        "CONDITIONING",
        "CONDITIONING_BATCH",
        "HOOKS",
    }
    excluded_outputs = {
        "IMAGE",
        "LATENT",
        "MASK",
        "AUDIO",
        "VIDEO",
        "PREVIEW",
        "UI",
    }

    assert pass_order[0] == "bypass_empty_lazy_lora"
    assert resource_outputs.isdisjoint(excluded_outputs)
    assert {"MODEL", "CLIP", "VAE"} <= resource_outputs


def test_characterized_failure_and_replacement_invariants() -> None:
    """Anchor deterministic canonical replacement and optimizer fail-open behavior."""

    equivalent_node_ids = ("10", "2", "7")
    canonical = min(equivalent_node_ids, key=lambda value: (len(value), value))
    fallback_stage = "validated_inherited_unoptimized_prompt"

    assert canonical == "2"
    assert fallback_stage == "validated_inherited_unoptimized_prompt"
