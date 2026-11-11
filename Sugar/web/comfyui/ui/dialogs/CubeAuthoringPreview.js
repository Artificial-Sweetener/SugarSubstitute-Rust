//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Render the complete derived Cube identity and graph-candidate summary. */
/** Keep authoring preview construction and literal-text updates cohesive. */
export class CubeAuthoringPreview {
    element;
    #defaultAlias;
    #identity;
    #filename;
    #targetModel;
    #destination;
    constructor(documentRef, candidate) {
        this.element = documentRef.createElement('div');
        this.element.className = 'sugarcubes-create-cube__preview';
        this.#defaultAlias = previewValue(documentRef);
        this.#identity = previewValue(documentRef, 'code');
        this.#filename = previewValue(documentRef);
        this.#targetModel = previewValue(documentRef);
        this.#destination = previewValue(documentRef);
        this.element.append(previewRow(documentRef, 'Default alias', this.#defaultAlias), previewRow(documentRef, 'Cube ID', this.#identity), previewRow(documentRef, 'Filename', this.#filename), previewRow(documentRef, 'Target model', this.#targetModel), previewRow(documentRef, 'Destination', this.#destination));
        const selection = selectionSummary(candidate);
        if (selection) {
            const value = previewValue(documentRef);
            value.textContent = selection;
            this.element.append(previewRow(documentRef, 'Selection', value));
        }
    }
    /** Refresh every derived preview field from one coherent authoring state. */
    update({ destination, identity, name, targetModel }) {
        this.#defaultAlias.textContent = identity?.defaultAlias || name || 'Name required';
        this.#identity.textContent =
            identity?.cubeId ||
                (!targetModel ? 'Target model required' : !name ? 'Name required' : 'Destination required');
        this.#filename.textContent = filename(identity?.cubeId);
        this.#targetModel.textContent = targetModel || 'Target model required';
        this.#destination.textContent = destination;
    }
}
/** Build one literal-text identity preview row. */
function previewRow(documentRef, label, value) {
    const row = documentRef.createElement('div');
    row.className = 'sugarcubes-create-cube__preview-row';
    const labelElement = documentRef.createElement('span');
    labelElement.className = 'sugarcubes-create-cube__label';
    labelElement.textContent = label;
    row.append(labelElement, value);
    return row;
}
/** Build one preview value using a safe element type. */
function previewValue(documentRef, tagName = 'span') {
    const value = documentRef.createElement(tagName);
    value.className = 'sugarcubes-create-cube__value';
    return value;
}
/** Summarize the executable nodes and native subgraph boundaries. */
function selectionSummary(candidate) {
    if (!candidate?.nodeIds && !candidate?.markerIds)
        return '';
    const nodeCount = candidate.nodeIds?.length ?? 0;
    if (candidate.inputCount != null || candidate.outputCount != null) {
        return `${nodeCount} nodes, ${candidate.inputCount ?? 0} inputs, ${candidate.outputCount ?? 0} outputs`;
    }
    return `${nodeCount} nodes, ${candidate.markerIds?.length ?? 0} markers`;
}
/** Return the filename segment of a canonical Cube identity. */
function filename(cubeId) {
    const segments = cubeId?.split('/') ?? [];
    return segments.at(-1) || 'cube.cube';
}
