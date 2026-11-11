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
/** Project negotiated native chrome clearance without changing saved appearance preferences. */
/** Retain and restore the document's exact pre-session viewport clearance. */
export class EmbeddedPanelLayout {
    window;
    #value;
    #priority;
    #size = null;
    /** Capture only the property owned by this native panel lifetime. */
    constructor(window) {
        this.window = window;
        const style = window.document.documentElement.style;
        this.#value = style.getPropertyValue('--sc-panel-resize-corner');
        this.#priority = style.getPropertyPriority('--sc-panel-resize-corner');
    }
    /** Notify geometry consumers only when the negotiated native corner changes. */
    apply(size) {
        if (size === this.#size)
            return;
        this.#size = size;
        this.window.document.documentElement.style.setProperty('--sc-panel-resize-corner', `${size}px`);
        this.window.dispatchEvent(new Event('resize'));
    }
    /** Restore local presentation after the native host disconnects. */
    dispose() {
        if (this.#size === null)
            return;
        this.#size = null;
        const style = this.window.document.documentElement.style;
        if (this.#value)
            style.setProperty('--sc-panel-resize-corner', this.#value, this.#priority);
        else
            style.removeProperty('--sc-panel-resize-corner');
        this.window.dispatchEvent(new Event('resize'));
    }
}
