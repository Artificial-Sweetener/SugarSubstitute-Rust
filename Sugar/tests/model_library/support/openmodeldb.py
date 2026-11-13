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
"""Provide exact OpenModelDB catalog evidence at the external transport seam."""

from __future__ import annotations


def catalog_payload() -> dict[str, object]:
    """Represent the reference fields with two exact formats and several provider listings."""
    return {
        "4x-Fixture": {
            "name": "<literal name>",
            "author": "<author>",
            "license": "CC-BY-4.0",
            "architecture": "span",
            "scale": 4,
            "tags": ["<tag>", " anime ", "", None],
            "description": "<literal description>",
            "date": "2000-01-01",
            "thumbnail": {"SR": "/thumbs/SR.png", "url": "/thumbs/LR.png"},
            "resources": [
                {
                    "type": "PTH",
                    "sha256": "A" * 64,
                    "size": 1024,
                    "urls": ["https://github.com/example/model.pth"],
                },
                {
                    "type": "SafeTensors",
                    "sha256": "B" * 64,
                    "size": 512,
                    "urls": ["https://huggingface.co/example/model.safetensors"],
                },
            ],
        },
        "2x-Duplicate": {
            "name": "Second listing",
            "resources": [
                {
                    "type": "pth",
                    "sha256": "A" * 64,
                    "size": 1024,
                    "urls": ["https://github.com/example/second.pth"],
                }
            ],
        },
    }
