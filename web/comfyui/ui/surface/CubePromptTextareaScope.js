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
/** Reconcile prompt autosizing within one renderer-owned DOM scope. */
import { CubePromptTextareaAutosizer } from './CubePromptTextareaAutosizer.js';
/** Own the set of prompt textareas currently rendered beneath one root. */
export class CubePromptTextareaScope {
    #root;
    #autosizer;
    #bound = new Set();
    /** Bind one independent native-card DOM scope. */
    constructor(root) {
        this.#root = root;
        this.#autosizer = new CubePromptTextareaAutosizer(root.ownerDocument);
    }
    /** Match autosizing ownership to the current semantic prompt presentation. */
    reconcile(enabled) {
        const desired = new Set(enabled ? this.#root.querySelectorAll('textarea') : []);
        for (const textarea of [...this.#bound]) {
            if (desired.has(textarea))
                continue;
            this.#autosizer.unbind(textarea);
            this.#bound.delete(textarea);
        }
        for (const textarea of desired) {
            if (this.#bound.has(textarea))
                continue;
            this.#autosizer.bind(textarea);
            this.#bound.add(textarea);
        }
    }
    /** Restore every textarea when its native card unmounts. */
    dispose() {
        this.#autosizer.dispose();
        this.#bound.clear();
    }
}
