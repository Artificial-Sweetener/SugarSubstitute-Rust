# SugarCubes

SugarCubes is a ComfyUI extension for authoring `.cube` files used by Sugar.

Sugar writes ComfyUI workflows as readable text. SugarCubes creates the reusable workflow units those scripts are built from.

A `.cube` is a saved ComfyUI subgraph with a stable public surface: named inputs, named outputs, editable controls, authored defaults, embedded node definitions, layout metadata, and versioned identity.

## What It Does

SugarCubes lets you build a reusable graph section in ComfyUI, mark its public boundary, and save it as a `.cube`.

Use it to:

- create cubes from ComfyUI graph sections
- define public cube inputs, outputs, and controls
- browse, preview, place, edit, and update cubes
- save cubes locally or into Git-backed cube packs
- preserve authored defaults, flavors, layout, and metadata
- manage cube libraries that Sugar can compile against

## Installation

Install SugarCubes through ComfyUI Manager.

Open Manager from the ComfyUI toolbar, click Custom Nodes Manager, search for SugarCubes, and click Install. Restart ComfyUI after installation.

## License

SugarCubes is licensed under the GNU Affero General Public License v3.0 or later. See [LICENSE](LICENSE).
