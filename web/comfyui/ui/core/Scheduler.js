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
 * Own the SugarCubes core UI service layer in `web/comfyui/ui/core/Scheduler.js`.
 */
/** Schedule cancellable UI work through the injected host timing boundary. */
export class Scheduler {
    adapter;
    constructor(adapter) {
        this.adapter = adapter;
    }
    raf(callback) {
        const raf = this.adapter?.getRaf?.() || null;
        return raf ? raf(callback) : null;
    }
    cancelRaf(id) {
        const cancel = this.adapter?.getCancelRaf?.() || null;
        if (cancel) {
            cancel(id);
        }
    }
    timeout(callback, delayMs) {
        const setTimer = this.adapter?.getSetTimeout?.() || null;
        return setTimer ? setTimer(callback, delayMs) : null;
    }
    clearTimeout(id) {
        const clearTimer = this.adapter?.getClearTimeout?.() || null;
        if (clearTimer) {
            clearTimer(id);
        }
    }
}
