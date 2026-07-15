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
 * Own the SugarCubes core UI service layer in `frontend/comfyui/ui/core/ToastService.js`.
 */
/**
 * Coordinate toast service behavior for the SugarCubes UI.
 */
export class ToastService {
    adapter;
    dialogs;
    constructor(adapter, { dialogs } = {}) {
        this.adapter = adapter;
        this.dialogs = dialogs || null;
    }
    push(severity, summary, detail) {
        const toast = this.adapter?.getToast?.() || null;
        if (toast?.add) {
            toast.add({ severity, summary, detail, life: 5000 });
            return;
        }
        const message = detail ? `${summary}: ${detail}` : summary;
        const consoleRef = this.adapter?.getConsole?.() || null;
        if (severity === 'error') {
            consoleRef?.error?.(message);
            this.dialogs
                ?.alert?.({
                title: summary || 'SugarCubes error',
                message: detail ? [detail] : [],
                confirmLabel: 'OK',
            })
                ?.catch?.((error) => consoleRef?.error?.(error));
        }
        else if (severity === 'warn' || severity === 'warning') {
            consoleRef?.warn?.(message);
        }
        else {
            consoleRef?.info?.(message);
        }
    }
}
