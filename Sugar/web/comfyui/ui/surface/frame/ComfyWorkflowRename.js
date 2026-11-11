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
/** Rename temporary workflows through Comfy's store without inventing file persistence or session identity. */
import { isRecord } from '../../types/common.js';
/** Adapt the host's draft rename capability while preserving closed-tab, collision and cancellation guards. */
export class ComfyWorkflowRename {
    store;
    open;
    prompt;
    /** Borrow the existing prompt and authoritative host workflow collection. */
    constructor(store, open, prompt) {
        this.store = store;
        this.open = open;
        this.prompt = prompt;
    }
    /** Override only the native unsaved-file restriction, retaining genuine host unavailability. */
    available(workflow) {
        return (workflow.isTemporary === true &&
            workflow.isPersisted === false &&
            this.store.isBusy !== true &&
            typeof this.store.renameWorkflow === 'function' &&
            typeof this.store.getWorkflowByPath === 'function' &&
            Array.isArray(this.store.workflows));
    }
    /** Preserve the workflow object and extension so sessions, drafts and output ownership follow the rename. */
    async rename(workflow) {
        if (!this.available(workflow) || !this.open().includes(workflow))
            throw new Error('The workflow is no longer available for renaming.');
        const { path, filename, fullFilename } = workflow;
        if (typeof path !== 'string' ||
            typeof filename !== 'string' ||
            typeof fullFilename !== 'string' ||
            !fullFilename.startsWith(filename))
            throw new Error('Comfy workflow filename metadata is unavailable.');
        const suffix = fullFilename.slice(filename.length);
        const directory = path.slice(0, path.lastIndexOf('/') + 1);
        const files = this.store.workflows;
        if (!Array.isArray(files) || !files.every(isRecord))
            throw new Error('Comfy workflow names are unavailable.');
        const occupied = new Set(files
            .filter((item) => item !== workflow)
            .map((item) => item.path)
            .filter((value) => typeof value === 'string')
            .map((value) => value.toLowerCase()));
        /** Keep an extension typed by the user from being appended a second time. */
        const destination = (value) => directory +
            (suffix && value.toLowerCase().endsWith(suffix.toLowerCase())
                ? value.slice(0, -suffix.length)
                : value) +
            suffix;
        /** Validate a filename rather than accepting a relative path from the text field. */
        const validate = (value) => {
            const name = value.trim();
            if (!name || name === '.' || name === '..')
                return 'Enter a workflow name.';
            if (/[<>:"/\\|?*]/.test(name) ||
                Array.from(name).some((character) => character.charCodeAt(0) < 32) ||
                name.endsWith('.') ||
                name.length + suffix.length > 255)
                return 'Use a valid filename without folders.';
            if (occupied.has(destination(name).toLowerCase()))
                return 'Another workflow already has this name.';
            return null;
        };
        const result = await this.prompt(filename, validate);
        if (result === null)
            return;
        const name = result.trim();
        const error = validate(name);
        if (error)
            throw new Error(error);
        if (!this.available(workflow) || !this.open().includes(workflow) || workflow.path !== path)
            throw new Error('The workflow changed while renaming it.');
        const next = destination(name);
        if (next === path)
            return;
        const lookup = this.store.getWorkflowByPath;
        const rename = this.store.renameWorkflow;
        if (typeof lookup !== 'function' || typeof rename !== 'function')
            throw new Error('Comfy workflow renaming is unavailable.');
        const collision = lookup.call(this.store, next);
        if (collision != null && collision !== workflow)
            throw new Error('Another workflow already has this name.');
        const renamed = rename.call(this.store, workflow, next);
        await renamed;
    }
}
