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
/** Ask for a mask-only canvas's new size and how its masks follow, through the modal shell. */
import { ModalShell } from '../../dialogs/ModalShell.js';
import { CANVAS_RESOLUTION_LIMITS } from '../application/WorkflowCanvasResolution.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
const ANCHORS = [
    'top-left',
    'top',
    'top-right',
    'left',
    'center',
    'right',
    'bottom-left',
    'bottom',
    'bottom-right',
];
/**
 * Offer "Resize canvas only" with a nine-point anchor (center by default) or "Scale canvas and
 * masks" with Nearest Neighbor or Smooth resampling (Smooth by default). Apply is enabled only
 * for a valid size that differs from the current one, and its label follows the chosen mode.
 */
export class CanvasResolutionDialog {
    document;
    #shell;
    /** Bind modal lifetime to the editor surface's document. */
    constructor(document) {
        this.document = document;
        this.#shell = new ModalShell({
            adapter: { getDocument: () => document, getWindow: () => document.defaultView },
            dialogClassName: 'sc-canvas-resolution',
        });
    }
    /** Resolve true after a successful apply and false on cancel. */
    async open(current, apply) {
        const copy = WORKFLOW_SURFACE_COPY;
        const form = this.document.createElement('form');
        form.className = 'sugarcubes-modal__form';
        const width = this.#number(current.width), height = this.#number(current.height);
        const scope = this.#select([
            ['canvas', copy.resizeCanvasOnly],
            ['scale', copy.scaleCanvasAndMasks],
        ]);
        const anchor = this.#select(ANCHORS.map((value) => [value, copy.anchors[value]]));
        anchor.value = 'center';
        const resampling = this.#select([
            ['nearest', copy.nearestNeighbor],
            ['smooth', copy.smooth],
        ]);
        resampling.value = 'smooth';
        const anchorField = this.#field(copy.anchorPoint, anchor), resamplingField = this.#field(copy.scalingQuality, resampling);
        form.append(this.#field(copy.width, width), this.#field(copy.height, height), this.#field(copy.resizeScope, scope), anchorField, resamplingField);
        const size = () => {
            const values = [Number(width.value), Number(height.value)];
            return values.every((value) => Number.isSafeInteger(value) &&
                value >= CANVAS_RESOLUTION_LIMITS.minimum &&
                value <= CANVAS_RESOLUTION_LIMITS.maximum)
                ? { width: values[0] ?? 0, height: values[1] ?? 0 }
                : null;
        };
        const mode = () => scope.value === 'scale'
            ? { kind: 'scale', resampling: resampling.value === 'nearest' ? 'nearest' : 'smooth' }
            : {
                kind: 'canvas',
                anchor: ANCHORS.find((value) => value === anchor.value) ?? 'center',
            };
        const update = () => {
            const scaled = scope.value === 'scale';
            anchorField.hidden = scaled;
            resamplingField.hidden = !scaled;
            const confirm = this.#shell.elements.confirmButton;
            if (confirm)
                confirm.textContent = scaled ? copy.scaleCanvas : copy.resizeCanvas;
            const next = size();
            this.#shell.setError('');
            this.#shell.setConfirmEnabled(Boolean(next && (next.width !== current.width || next.height !== current.height)));
        };
        for (const control of [width, height, scope, anchor, resampling]) {
            control.addEventListener('input', update);
            control.addEventListener('change', update);
        }
        form.addEventListener('submit', (event) => {
            event.preventDefault();
            this.#shell.elements.confirmButton?.click();
        });
        let busy = false;
        const result = this.#shell.open({
            title: copy.changeCanvasResolution,
            description: copy.changeCanvasResolutionDescription,
            body: form,
            confirmLabel: copy.resizeCanvas,
            cancelLabel: copy.cancel,
            cancelResult: false,
            initialFocus: () => width,
            onConfirm: () => {
                const next = size();
                if (!next || busy)
                    return;
                busy = true;
                this.#shell.setBusy(true);
                apply(next.width, next.height, mode()).then(() => this.#shell.close(true), (error) => {
                    busy = false;
                    this.#shell.setBusy(false);
                    this.#shell.setError(error instanceof Error ? error.message : copy.resizeFailed);
                });
            },
        });
        update();
        return (await result) === true;
    }
    /** Cancel an open dialog during shutdown. */
    close() {
        this.#shell.close(false);
    }
    /** Build an integer pixel field within the offered range. */
    #number(value) {
        const input = this.document.createElement('input');
        input.className = 'p-inputtext p-component sugarcubes-modal__text-input';
        input.type = 'number';
        input.min = String(CANVAS_RESOLUTION_LIMITS.minimum);
        input.max = String(CANVAS_RESOLUTION_LIMITS.maximum);
        input.step = String(CANVAS_RESOLUTION_LIMITS.step);
        input.value = String(value);
        return input;
    }
    /** Build a literal choice list. */
    #select(options) {
        const select = this.document.createElement('select');
        select.className = 'p-inputtext p-component sugarcubes-modal__text-input';
        for (const [value, label] of options) {
            const option = this.document.createElement('option');
            option.value = value;
            option.textContent = label;
            select.append(option);
        }
        return select;
    }
    /** Label one control the way other SugarCubes forms do. */
    #field(caption, control) {
        const label = this.document.createElement('label');
        label.className = 'sugarcubes-modal__field';
        const text = this.document.createElement('span');
        text.className = 'sugarcubes-modal__field-label';
        text.textContent = caption;
        label.append(text, control);
        return label;
    }
}
