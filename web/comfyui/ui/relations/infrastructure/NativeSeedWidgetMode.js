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
/** Reconcile Comfy's uncontrolled local mode ref with externally restored seed policy. */
import { isRecord } from '../../types/common.js';
import { isSeedMode } from '../../graph/ComfySeedMode.js';
/** Retain native popup identity for native edits and reinitialize only stale visible mode refs. */
export class NativeSeedWidgetMode {
    #mode = null;
    #revision = 0;
    /** Read a stable component identity for unchanged values and native mode interactions. */
    get key() {
        return this.#revision;
    }
    /** Track host-originated mode writes before their synchronous parent refresh can render. */
    prepare(widget, seed, fluent) {
        const expected = seed.readMode();
        if (this.#mode === null)
            this.#mode = expected;
        if (!fluent && this.#mode !== expected) {
            this.#mode = expected;
            this.#revision++;
        }
        const control = isRecord(widget.controlWidget) ? widget.controlWidget : null;
        const update = control?.update;
        if (control && typeof update === 'function')
            control.update = (mode) => {
                if (isSeedMode(mode))
                    this.#mode = mode;
                update.call(control, mode);
            };
        return this.#revision;
    }
}
