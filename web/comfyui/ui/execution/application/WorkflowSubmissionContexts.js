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
/** Associate exact constructed objects with immutable entry-time workflow ownership. */
import { freezeWorkflowSubmissionContext } from '../domain/WorkflowSubmission.js';
/** Retain lightweight associations weakly so batch reuse requires neither guessing nor a strong cache. */
export class WorkflowSubmissionContexts {
    #contexts = new WeakMap();
    /** Bind the original result object without adding fields to persisted workflow data. */
    associate(prompt, session, sources) {
        this.#contexts.set(prompt, freezeWorkflowSubmissionContext({ session, sources }));
    }
    /** Give one scene's prompt the original prompt's ownership, qualified by its scene. */
    associateScene(prompt, original, scene) {
        const context = this.#contexts.get(original);
        if (context)
            this.#contexts.set(prompt, freezeWorkflowSubmissionContext({ ...context, scene }));
    }
    /** Resolve only the exact known prompt object, including explicit reuse for multiple batch requests. */
    read(prompt) {
        return this.#contexts.get(prompt) ?? null;
    }
    /** Retire every association when the owning integration lifetime ends. */
    dispose() {
        this.#contexts = new WeakMap();
    }
}
