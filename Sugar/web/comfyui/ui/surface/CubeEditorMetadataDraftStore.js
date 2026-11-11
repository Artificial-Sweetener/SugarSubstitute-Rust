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
/** Retain unsaved Cube metadata independently from viewport HUD visibility. */
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
/** Own transient authoring drafts for one Cube editor runtime session. */
export class CubeEditorMetadataDraftStore {
    #drafts = new Map();
    /** Return an isolated draft for one stable Cube instance. */
    get(node) {
        const draft = this.#drafts.get(readInstanceId(node));
        return draft ? cloneDraft(draft) : null;
    }
    /** Retain one dirty draft while its HUD is temporarily hidden. */
    set(node, draft) {
        this.#drafts.set(readInstanceId(node), cloneDraft(draft));
    }
    /** Forget transient state after the authoritative save succeeds. */
    delete(node) {
        this.#drafts.delete(readInstanceId(node));
    }
    /** Release every draft when the owning editor runtime is disposed. */
    clear() {
        this.#drafts.clear();
    }
}
/** Clone mutable arrays so HUD rendering cannot mutate retained state. */
function cloneDraft(draft) {
    return {
        ...draft,
        values: {
            ...draft.values,
            supportedModels: [...draft.values.supportedModels],
        },
    };
}
