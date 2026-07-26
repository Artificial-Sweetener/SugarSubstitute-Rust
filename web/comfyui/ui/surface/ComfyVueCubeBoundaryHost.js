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
/** Position graph-owned Nodes 2.0 slots on animated Cube boundary gutters. */
import { measureCubeBoundary, measureSocketRadius, readLocalOffset, readVueNodeGraphY, } from './CubeDomBoundaryGeometry.js';
import { CubeDomPortLeaderHost } from './CubeDomPortLeaderHost.js';
/** Own only structural positioning for Comfy's actual native slot elements. */
export class ComfyVueCubeBoundaryHost {
    #body;
    #node;
    #titleHeight;
    #portPresentation;
    #requestSlotLayoutSync;
    #resolveOriginY;
    #presentations = new Map();
    #inputLabels = new Map();
    #leaders;
    #unsubscribe;
    #measurements = {
        input: null,
        output: null,
    };
    #lastSyncedLayout = '';
    /** Bind one current native node body. */
    constructor(options) {
        this.#body = options.body;
        this.#node = options.node;
        this.#titleHeight = Math.max(0, Number(options.titleHeight) || 30);
        this.#portPresentation = options.portPresentation ?? null;
        this.#requestSlotLayoutSync = options.requestSlotLayoutSync ?? (() => undefined);
        const nodeRoot = this.#body.closest('.lg-node') ?? this.#body;
        this.#resolveOriginY = () => readVueNodeGraphY(nodeRoot, this.#titleHeight, Number(this.#node.pos?.[1]) || 0);
        this.#portPresentation?.registerOrigin(this.#node, this.#resolveOriginY);
        this.#leaders = new CubeDomPortLeaderHost(options.body);
        this.#unsubscribe =
            this.#portPresentation?.subscribe(options.node, () => this.#renderAnimationFrame()) ??
                (() => undefined);
        this.reconcile();
    }
    /** Reapply markers after Comfy replaces slot descendants. */
    reconcile() {
        this.#portPresentation?.registerOrigin(this.#node, this.#resolveOriginY);
        const row = [...this.#body.children].find((child) => child instanceof HTMLElement && child.querySelector('.lg-slot') !== null);
        if (!row)
            return;
        const inputSlots = [...row.querySelectorAll('.lg-slot--input')];
        const outputSlots = [...row.querySelectorAll('.lg-slot--output')];
        const outputTitles = measureOutputTitles(this.#body, row, outputSlots.length);
        const nodeRoot = this.#body.closest('.lg-node') ?? this.#body;
        const priorInputMeasurement = this.#measurements.input;
        const stableInputRowPositions = priorInputMeasurement?.anchors.length === inputSlots.length
            ? priorInputMeasurement.anchors.map((anchor) => anchor.defaultY +
                priorInputMeasurement.graphOriginOffset -
                priorInputMeasurement.rowOffset)
            : [];
        this.#measurements.input = measureCubeBoundary(this.#body, nodeRoot, row, inputSlots, stableInputRowPositions, this.#titleHeight);
        this.#measurements.output = measureCubeBoundary(this.#body, nodeRoot, row, outputSlots, outputTitles.map((item) => item.y), this.#titleHeight);
        this.#remember(row);
        row.dataset.sugarcubeBoundaryRow = '';
        this.#markSlots('input', inputSlots, true);
        this.#markSlots('output', outputSlots, true);
        this.#renderLeaders(row, outputSlots, outputTitles);
        this.#requestLayoutSyncForNewGeometry(inputSlots, outputSlots);
    }
    /** Paint transient positions without feeding animated geometry back into anchor measurement. */
    #renderAnimationFrame() {
        const row = [...this.#body.children].find((child) => child instanceof HTMLElement && child.querySelector('.lg-slot') !== null);
        if (!row)
            return;
        const inputSlots = [...row.querySelectorAll('.lg-slot--input')];
        const outputSlots = [...row.querySelectorAll('.lg-slot--output')];
        const outputTitles = measureOutputTitles(this.#body, row, outputSlots.length);
        this.#markSlots('input', inputSlots, false);
        this.#markSlots('output', outputSlots, false);
        this.#renderLeaders(row, outputSlots, outputTitles);
        this.#requestLayoutSyncForNewGeometry(inputSlots, outputSlots);
    }
    /** Ask Comfy to remeasure once per semantic boundary geometry revision. */
    #requestLayoutSyncForNewGeometry(inputSlots, outputSlots) {
        if (this.#portPresentation?.isAnimating(this.#node))
            return;
        const signature = [
            serializeSlotPositions('input', inputSlots),
            serializeSlotPositions('output', outputSlots),
        ].join('|');
        if (signature === this.#lastSyncedLayout)
            return;
        this.#lastSyncedLayout = signature;
        this.#requestSlotLayoutSync();
    }
    /** Update stable leader elements from current title and socket positions. */
    #renderLeaders(row, outputSlots, outputTitles) {
        const rowOffset = readLocalOffset(this.#body, row);
        this.#leaders.render(outputTitles.flatMap((item, index) => {
            const portY = readBoundaryPosition(outputSlots[index]);
            return portY === null
                ? []
                : [
                    {
                        index,
                        title: item.title,
                        portY: rowOffset + portY,
                        socketRadius: measureSocketRadius(this.#body, outputSlots[index]),
                    },
                ];
        }));
    }
    /** Restore every Comfy-owned element exactly as it was mounted. */
    dispose() {
        this.#unsubscribe();
        this.#leaders.dispose();
        for (const [label, text] of this.#inputLabels)
            label.textContent = text;
        this.#inputLabels.clear();
        for (const [element, presentation] of this.#presentations) {
            restoreAttribute(element, 'style', presentation.style);
            restoreDataset(element, 'sugarcubeBoundaryRow', presentation.boundaryRow);
            restoreDataset(element, 'sugarcubeBoundaryDirection', presentation.boundaryDirection);
            restoreDataset(element, 'sugarcubeBoundaryIndex', presentation.boundaryIndex);
        }
        this.#presentations.clear();
    }
    /** Mark one direction's native slots from its stable measured geometry. */
    #markSlots(direction, slots, registerAnchors) {
        const measurement = this.#measurements[direction];
        if (!measurement)
            return false;
        if (registerAnchors) {
            this.#portPresentation?.register(this.#node, direction, measurement.anchors);
        }
        let changed = false;
        for (const [index, slot] of slots.entries()) {
            this.#remember(slot);
            const boundaryIndex = String(index);
            const boundaryPosition = (this.#portPresentation?.resolveLocalY(this.#node, direction, index) ??
                measurement.anchors[index]?.defaultY ??
                measurement.rowMinimumY + measurement.rowOffset - measurement.graphOriginOffset) +
                measurement.graphOriginOffset -
                measurement.rowOffset;
            const boundaryPositionCss = `${String(clamp(boundaryPosition, measurement.rowMinimumY, measurement.rowMaximumY))}px`;
            changed =
                slot.dataset.sugarcubeBoundaryDirection !== direction ||
                    slot.dataset.sugarcubeBoundaryIndex !== boundaryIndex ||
                    slot.style.getPropertyValue('--sugarcube-boundary-position') !== boundaryPositionCss ||
                    changed;
            slot.dataset.sugarcubeBoundaryDirection = direction;
            slot.dataset.sugarcubeBoundaryIndex = boundaryIndex;
            slot.style.setProperty('--sugarcube-boundary-position', boundaryPositionCss);
            if (direction === 'input')
                this.#presentInputLabel(slot, index);
        }
        return changed;
    }
    /** Render a concise type label without mutating native slot metadata. */
    #presentInputLabel(slot, index) {
        const label = slot.querySelector('.text-node-component-slot-text');
        if (!label)
            return;
        if (!this.#inputLabels.has(label))
            this.#inputLabels.set(label, label.textContent);
        const type = readPortType(this.#node.inputs?.[index]?.type);
        if (type && label.textContent !== type)
            label.textContent = type;
    }
    /** Retain host presentation once before applying Cube markers. */
    #remember(element) {
        if (this.#presentations.has(element))
            return;
        this.#presentations.set(element, {
            style: element.getAttribute('style'),
            boundaryRow: element.dataset.sugarcubeBoundaryRow,
            boundaryDirection: element.dataset.sugarcubeBoundaryDirection,
            boundaryIndex: element.dataset.sugarcubeBoundaryIndex,
        });
    }
}
/** Restore one nullable host attribute. */
function restoreAttribute(element, name, value) {
    if (value === null) {
        element.removeAttribute(name);
    }
    else {
        element.setAttribute(name, value);
    }
}
/** Restore one optional host dataset value. */
function restoreDataset(element, key, value) {
    if (value === undefined) {
        element.removeAttribute(DATASET_ATTRIBUTES[key]);
    }
    else {
        element.dataset[key] = value;
    }
}
const DATASET_ATTRIBUTES = {
    sugarcubeBoundaryRow: 'data-sugarcube-boundary-row',
    sugarcubeBoundaryDirection: 'data-sugarcube-boundary-direction',
    sugarcubeBoundaryIndex: 'data-sugarcube-boundary-index',
};
/** Measure preview-title centers in the row's unscaled CSS coordinate space. */
function measureOutputTitles(body, row, count) {
    const titles = [...body.querySelectorAll('[data-cube-preview-output-title]')].slice(0, count);
    if (titles.length !== count || count === 0)
        return [];
    const bodyRect = body.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const scale = body.offsetWidth > 0 ? bodyRect.width / body.offsetWidth : 1;
    const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
    return titles.map((title) => {
        const titleRect = title.getBoundingClientRect();
        const position = (titleRect.top + titleRect.height / 2 - rowRect.top) / safeScale;
        return { title, y: Math.max(0, position) };
    });
}
/** Read one currently applied pixel position for output leader geometry. */
function readBoundaryPosition(slot) {
    const value = slot?.style.getPropertyValue('--sugarcube-boundary-position');
    if (!value)
        return null;
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : null;
}
/** Serialize stable presentation geometry independently from Vue element identity. */
function serializeSlotPositions(direction, slots) {
    return `${direction}:${slots
        .map((slot) => slot.style.getPropertyValue('--sugarcube-boundary-position'))
        .join(',')}`;
}
/** Read one concise native type while treating host text as untrusted content. */
function readPortType(value) {
    if (typeof value === 'string')
        return value.trim();
    if (!Array.isArray(value))
        return '';
    return value
        .filter((item) => typeof item === 'string')
        .map((item) => item.trim())
        .filter(Boolean)
        .join(' | ');
}
/** Clamp one rendered row coordinate to its measured Cube-body travel range. */
function clamp(value, minimum, maximum) {
    return Math.min(Math.max(value, minimum), maximum);
}
