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
"""Expose only generated qualification media through a disposable ComfyUI test node."""

from __future__ import annotations

from typing import ClassVar


class SyntheticMediaOutput:
    """Emit deterministic fixture references without loading models or personal files."""

    RETURN_TYPES: ClassVar[tuple[str, ...]] = ()
    FUNCTION: ClassVar[str] = "execute"
    OUTPUT_NODE: ClassVar[bool] = True
    CATEGORY: ClassVar[str] = "testing/SugarCubes"

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, object]:
        """Offer only explicit synthetic scenarios for browser qualification."""
        return {
            "required": {
                "content": (
                    [
                        "mixed",
                        "image",
                        "video",
                        "model",
                        "animated",
                        "missing-video",
                        "missing-model",
                    ],
                ),
                "count": ("INT", {"default": 1, "min": 1, "max": 32}),
            }
        }

    def execute(self, content: str, count: int) -> dict[str, object]:
        """Publish fixture files exclusively within the configured synthetic output root."""
        files = {
            "image": "still.png",
            "video": "clip.webm",
            "model": "tetrahedron.glb",
            "animated": "animation.gif",
            "missing-video": "missing.webm",
            "missing-model": "missing.glb",
        }
        kinds = (
            ["image", "video", "animated", "model"] if content == "mixed" else [content]
        )
        images: list[dict[str, object]] = []
        models: list[dict[str, object]] = []
        videos: list[dict[str, object]] = []
        for _ in range(count):
            for kind in kinds:
                reference: dict[str, object] = {
                    "filename": files[kind],
                    "subfolder": "synthetic",
                    "type": "output",
                }
                (
                    models
                    if kind in {"model", "missing-model"}
                    else videos if content == "mixed" and kind == "video" else images
                ).append(reference)
        payload: dict[str, object] = {"images": images}
        if models:
            payload["3d"] = models
        if videos:
            payload["video"] = videos
        if content in {"video", "animated", "missing-video"}:
            payload["animated"] = [True]
        return {"ui": payload, "result": ()}


NODE_CLASS_MAPPINGS = {"SugarCubesSyntheticMedia": SyntheticMediaOutput}
NODE_DISPLAY_NAME_MAPPINGS = {"SugarCubesSyntheticMedia": "Synthetic Media Output"}
