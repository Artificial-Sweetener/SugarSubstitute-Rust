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
"""Keep image consumers independent of per-run persistence side-effect cache keys."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy

from ...execution.models import ApiPrompt


def bypass_image_sink_dependencies(
    prompt: ApiPrompt, image_inputs: Mapping[str, object]
) -> None:
    """Project verified image passthrough links onto their upstream tensors in the execution copy.

    Saving sinks remain output nodes with their original identities. Consumers
    depend directly on unchanged images so fresh saving tickets cannot force
    downstream samplers or image processors to regenerate.
    """
    for node in prompt.values():
        inputs = node.get("inputs")
        if not isinstance(inputs, dict):
            continue
        for name, value in tuple(inputs.items()):
            visited: set[str] = set()
            redirected = value
            while (
                isinstance(redirected, list)
                and len(redirected) == 2
                and isinstance(redirected[0], str)
                and redirected[1] == 0
                and redirected[0] in image_inputs
            ):
                source_id = redirected[0]
                if source_id in visited:
                    raise ValueError(
                        "Image saving passthrough contains cyclic dependencies."
                    )
                visited.add(source_id)
                redirected = image_inputs[source_id]
            if visited:
                inputs[name] = deepcopy(redirected)
