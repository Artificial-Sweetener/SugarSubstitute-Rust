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
/**
 * Keep prompt-editor keystrokes out of LiteGraph's document-wide keyup handling.
 *
 * LiteGraph registers `canvas._key_callback` (its bound `processKey`) as a capture-phase keyup
 * listener on the canvas document and ends every keyup it sees with `graph.change()`, which
 * repaints the whole canvas. It skips only `<input>` targets, so each keystroke typed into the
 * prompt editor repainted the graph. This boundary is the single owner of that private
 * contract: it re-registers the callback behind a filter that withholds editor keyups, and
 * reports and leaves LiteGraph untouched when the contract is not the one it was written for.
 */
import { isRecord } from '../types/common.js';
const EDITOR_SELECTOR = '[data-sc-prompt-editor]';
/** Withhold editor keyups from LiteGraph while every other listener still receives them. */
export class ComfyCanvasEditorKeyBoundary {
    app;
    document;
    report;
    #canvas = null;
    #original = null;
    #filter = null;
    /** Bind one canvas document lifetime and its contract-failure reporter. */
    constructor(app, document, report) {
        this.app = app;
        this.document = document;
        this.report = report;
    }
    /** Replace LiteGraph's keyup registration with the filtered one; idempotent. */
    install() {
        if (this.#filter)
            return;
        const canvas = isRecord(this.app) && isRecord(this.app.canvas) ? this.app.canvas : null;
        const original = canvas?._key_callback;
        if (!canvas || typeof original !== 'function') {
            this.report(new TypeError('LiteGraph canvas keyup contract changed: app.canvas._key_callback is not a function; ' +
                'prompt-editor keystrokes will repaint the graph canvas.'));
            return;
        }
        const callback = original;
        const filter = (event) => {
            // LiteGraph rebinding its events registers a new callback itself; this stale filter retires.
            if (canvas._key_callback !== callback) {
                this.#retire();
                this.report(new TypeError('LiteGraph rebound its canvas keyup callback; prompt-editor keystrokes will repaint ' +
                    'the graph canvas until reload.'));
                return;
            }
            if (this.#withheld(event, canvas))
                return;
            callback(event);
        };
        this.document.removeEventListener('keyup', callback, true);
        this.document.addEventListener('keyup', filter, true);
        this.#canvas = canvas;
        this.#original = callback;
        this.#filter = filter;
    }
    /** Restore LiteGraph's own registration while it still owns the same callback. */
    dispose() {
        const canvas = this.#canvas, original = this.#original;
        if (!this.#retire())
            return;
        if (canvas && original && canvas._key_callback === original)
            this.document.addEventListener('keyup', original, true);
    }
    /** Editor keyups carry no graph meaning, except ending a space-drag LiteGraph began. */
    #withheld(event, canvas) {
        const target = event.target;
        if (!(target instanceof Element) || !target.closest(EDITOR_SELECTOR))
            return false;
        return !(event.key === ' ' && canvas.read_only === true);
    }
    /** Remove the filter; report whether one was installed. */
    #retire() {
        const filter = this.#filter;
        if (!filter)
            return false;
        this.document.removeEventListener('keyup', filter, true);
        this.#filter = null;
        this.#canvas = null;
        this.#original = null;
        return true;
    }
}
