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
/** Own host feedback. */
/** Route host feedback through one owner. */
export class CubeHostFeedback {
    #toast;
    constructor(options) {
        this.#toast = options.toast;
    }
    /** Return a stable display message for an unknown failure. */
    readErrorMessage(error) {
        return error instanceof Error ? error.message : String(error);
    }
    /** Push one host-facing toast through the configured feedback service. */
    pushToast(severity, summary, detail) {
        this.#toast?.push?.(severity, summary, detail);
    }
}
