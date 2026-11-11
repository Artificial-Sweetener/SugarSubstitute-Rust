#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Build compact native workflows for execution-domain contract tests."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping, Sequence


def cube_document(
    name: str,
    *,
    nodes: Mapping[str, object],
    inputs: Mapping[str, object] | None = None,
    outputs: Mapping[str, object] | None = None,
    definitions: Mapping[str, object] | None = None,
    subgraphs: Sequence[Mapping[str, object]] = (),
) -> dict[str, object]:
    """Return one canonical Cube document with an execution-focused graph."""

    implementation_nodes = deepcopy(dict(nodes))
    implementation_inputs = deepcopy(dict(inputs)) if inputs is not None else None
    implementation_outputs = deepcopy(dict(outputs)) if outputs is not None else None
    implementation_definitions = deepcopy(dict(definitions or RESOURCE_DEFINITIONS))
    if implementation_inputs is None and implementation_outputs is None:
        implementation_nodes["__topology__"] = {
            "class_type": "TopologyRelay",
            "inputs": {"resource": name},
        }
        implementation_inputs = {
            "input.resource": {
                "kind": "input",
                "targets": [["__topology__", "resource"]],
            }
        }
        implementation_outputs = {"output.resource": ["__topology__", 0]}

    return {
        "cube_id": f"Artificial-Sweetener/Test/{name}.cube",
        "version": "1.0.0",
        "description": f"Execution fixture {name}.",
        "metadata": {"default_alias": name},
        "implementation": {
            "nodes": implementation_nodes,
            "inputs": implementation_inputs or {},
            "outputs": implementation_outputs or {},
            "layout": {},
            "definitions": implementation_definitions,
            "subgraphs": deepcopy(list(subgraphs)),
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


def cube_workflow(
    documents: Mapping[str, Mapping[str, object]],
    *,
    links: Sequence[object] = (),
    loose_nodes: Sequence[Mapping[str, object]] = (),
    modes: Mapping[str, int] | None = None,
) -> dict[str, object]:
    """Embed documents as native definitions and matching root Cube nodes."""

    nodes: list[dict[str, object]] = []
    definitions: list[dict[str, object]] = []
    for index, (instance_id, document_value) in enumerate(documents.items(), start=1):
        document = deepcopy(dict(document_value))
        cube_id = str(document["cube_id"])
        cube_version = str(document["version"])
        alias = str(document["metadata"]["default_alias"])  # type: ignore[index]
        definition_id = f"definition-{instance_id}"
        implementation = document["implementation"]
        assert isinstance(implementation, Mapping)
        input_names = list(_mapping(implementation.get("inputs"))) or ["input.resource"]
        output_names = list(_mapping(implementation.get("outputs"))) or [
            "output.resource"
        ]
        identity = {
            "cube_id": cube_id,
            "cube_version": cube_version,
            "default_alias": alias,
        }
        nodes.append(
            {
                "id": index,
                "type": definition_id,
                "mode": (modes or {}).get(instance_id, 0),
                "inputs": [{"name": name, "type": "*"} for name in input_names],
                "outputs": [{"name": name, "type": "*"} for name in output_names],
                "properties": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": {
                        **identity,
                        "instance_id": instance_id,
                        "instance_alias": alias,
                    },
                },
            }
        )
        definitions.append(
            {
                "id": definition_id,
                "name": alias,
                "nodes": [],
                "links": [],
                "inputs": [{"name": name, "type": "*"} for name in input_names],
                "outputs": [{"name": name, "type": "*"} for name in output_names],
                "extra": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": identity,
                    "sugarcubes_document": document,
                },
            }
        )
    nodes.extend(deepcopy([dict(node) for node in loose_nodes]))
    return {
        "version": 0.4,
        "nodes": nodes,
        "links": deepcopy(list(links)),
        "definitions": {"subgraphs": definitions},
        "extra": {"fixture": True},
    }


def provider_document(name: str = "Provider") -> dict[str, object]:
    """Return one Cube with live MODEL, CLIP, and VAE origin providers."""

    return cube_document(
        name,
        nodes={
            "checkpoint": {"class_type": "CheckpointLoader", "inputs": {}},
            "model_use": {
                "class_type": "ModelConsumer",
                "inputs": {"model": ["checkpoint", 0]},
            },
            "clip_use": {
                "class_type": "ClipConsumer",
                "inputs": {"clip": ["checkpoint", 1]},
            },
            "vae_use": {
                "class_type": "VaeConsumer",
                "inputs": {"vae": ["checkpoint", 2]},
            },
        },
    )


def consumer_document(name: str = "Consumer") -> dict[str, object]:
    """Return one Cube with unresolved inheritable resource inputs."""

    return cube_document(
        name,
        nodes={
            "model_use": {"class_type": "ModelConsumer", "inputs": {"model": None}},
            "clip_use": {"class_type": "ClipConsumer", "inputs": {"clip_in": None}},
            "vae_use": {"class_type": "VaeConsumer", "inputs": {"vae": ""}},
        },
    )


def image_passthrough_document(name: str) -> dict[str, object]:
    """Return one Cube with a public IMAGE input and output."""

    return cube_document(
        name,
        nodes={"image": {"class_type": "ImagePass", "inputs": {"image": None}}},
        inputs={"input.image": {"kind": "input", "targets": [["image", "image"]]}},
        outputs={"output.image": ["image", 0]},
    )


RESOURCE_DEFINITIONS: dict[str, object] = {
    "CheckpointLoader": {
        "input": {"required": {}},
        "output": ["MODEL", "CLIP", "VAE"],
        "output_name": ["MODEL", "CLIP", "VAE"],
    },
    "ModelConsumer": {
        "input": {"required": {"model": ["MODEL"], "model_in": ["MODEL"]}},
        "output": ["IMAGE"],
    },
    "ClipConsumer": {
        "input": {"required": {"clip": ["CLIP"], "clip_in": ["CLIP"]}},
        "output": ["CONDITIONING"],
    },
    "VaeConsumer": {
        "input": {"required": {"vae": ["VAE"]}},
        "output": ["IMAGE"],
    },
    "ModelTransform": {
        "input": {"required": {"model": ["MODEL"]}},
        "output": ["MODEL"],
        "output_name": ["MODEL"],
    },
    "ImagePass": {
        "input": {"required": {"image": ["IMAGE"]}},
        "output": ["IMAGE"],
    },
    "TopologyRelay": {
        "input": {"required": {"resource": ["STRING"]}},
        "output": ["STRING"],
    },
}


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow one fixture object without broad casting."""

    assert isinstance(value, Mapping)
    return value
