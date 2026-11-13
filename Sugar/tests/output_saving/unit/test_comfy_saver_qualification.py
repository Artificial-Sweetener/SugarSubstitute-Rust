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
"""Prove broken custom-node inspection remains observable and never qualifies a saving adapter."""

from types import ModuleType
import sys

import pytest

from sugarcubes.output_saving.infrastructure.comfy_node_capabilities import (
    ComfySavingNodeCapabilities,
)
from sugarcubes.output_saving.domain.run_plan import SourceKind
from sugarcubes.output_saving.domain.saver_contract import ImageSaverContract


def test_arbitrary_custom_schema_failure_is_logged_and_remains_unqualified(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    """Preserve custom-node execution behavior when optional image-output discovery cannot inspect it."""

    class SchemaFailure(Exception):
        """Represent a host-extension exception outside known built-in error families."""

    class BrokenOutput:
        """Expose a native output node whose external schema query fails."""

        OUTPUT_NODE = True

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Fail at the trusted custom-node adapter boundary."""
            raise SchemaFailure("fixture schema failure")

    module = ModuleType("nodes")
    setattr(module, "NODE_CLASS_MAPPINGS", {"BrokenOutput": BrokenOutput})
    monkeypatch.setitem(sys.modules, "nodes", module)
    capabilities = ComfySavingNodeCapabilities()
    assert not capabilities.qualified_class("BrokenOutput")
    assert not capabilities.image_output_class("BrokenOutput")
    assert "Could not classify native output-node image inputs" in caplog.text


@pytest.mark.parametrize("kind", ["save", "preview"])
def test_explicit_custom_author_contract_qualifies_an_alternate_image_input(
    monkeypatch: pytest.MonkeyPatch, kind: str
) -> None:
    """Accept an explicit unchanged-image contract rather than guessing from names or filename widgets."""

    class CustomOutput:
        """Delegate this declared image passthrough sink's saving policy to SugarCubes."""

        OUTPUT_NODE = True
        RETURN_TYPES = ("IMAGE",)
        SUGARCUBES_IMAGE_SAVER = {
            "schema_version": 1,
            "mode": "replace_image_passthrough",
            "image_input": "picture",
            "kind": kind,
        }

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Expose the author-declared input at the trusted class boundary."""
            return {"required": {"picture": ("IMAGE",), "prefix": ("STRING",)}}

    module = ModuleType("nodes")
    setattr(
        module,
        "NODE_CLASS_MAPPINGS",
        {"AuthorSaver": CustomOutput, "SaveImage": CustomOutput},
    )
    monkeypatch.setitem(sys.modules, "nodes", module)
    capabilities = ComfySavingNodeCapabilities()
    assert capabilities.qualified_class("AuthorSaver") == ImageSaverContract(
        "AuthorSaver", "picture", SourceKind(kind)
    )
    assert capabilities.qualified_class("SaveImage") is None
    assert capabilities.image_output_class("AuthorSaver")


@pytest.mark.parametrize(
    "declaration, returns",
    [
        ({"schema_version": True}, ("IMAGE",)),
        ({"schema_version": 2}, ("IMAGE",)),
        (
            {
                "schema_version": 1,
                "mode": "replace_image_passthrough",
                "image_input": "picture",
                "kind": [],
            },
            ("IMAGE",),
        ),
        (
            {
                "schema_version": 1,
                "mode": "replace_image_passthrough",
                "image_input": "missing",
                "kind": "save",
            },
            ("IMAGE",),
        ),
        (
            {
                "schema_version": 1,
                "mode": "replace_image_passthrough",
                "image_input": "picture",
                "kind": "save",
            },
            ("IMAGE", "MASK"),
        ),
    ],
)
def test_unsupported_custom_contract_keeps_native_side_effects(
    monkeypatch: pytest.MonkeyPatch,
    declaration: dict[str, object],
    returns: tuple[str, ...],
) -> None:
    """Reject incompatible versions, image inputs and extra results without registering a replacement."""

    class CustomOutput:
        """Expose a deliberately incompatible author declaration."""

        OUTPUT_NODE = True
        RETURN_TYPES = returns
        SUGARCUBES_IMAGE_SAVER = declaration

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Keep the native node discoverable even when its saving declaration is unsupported."""
            return {"required": {"picture": ("IMAGE",)}}

    module = ModuleType("nodes")
    setattr(module, "NODE_CLASS_MAPPINGS", {"Custom": CustomOutput})
    monkeypatch.setitem(sys.modules, "nodes", module)
    assert ComfySavingNodeCapabilities().qualified_class("Custom") is None
    assert ComfySavingNodeCapabilities().image_output_class("Custom")
