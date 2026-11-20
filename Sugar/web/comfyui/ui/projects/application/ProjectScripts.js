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
/** Open and save project scripts the way SugarSubstitute did, over ports this owner does not implement. */
import { isProjectScript, namedScriptPath, projectDocument, projectNameError, projectNameOf, projectOwnerOf, projectScriptPath, uniqueTabName, versionPath, } from '../domain/ProjectScript.js';
const MOVE_ATTEMPTS = 100;
/**
 * Open, save and save-as project scripts.
 *
 * A project is the folder named after its workflow tab. Saving always writes
 * `<tab name>/<tab name>.sugar` and first keeps the copy it replaces under `versions/`; the
 * script itself carries the tab's name in its header.
 */
export class ProjectScripts {
    files;
    workflows;
    render;
    masks;
    now;
    constructor(files, workflows, render, masks = null, now = () => new Date()) {
        this.files = files;
        this.workflows = workflows;
        this.render = render;
        this.masks = masks;
        this.now = now;
    }
    /** List the scripts a person can open, by path, most recently written first. */
    async scripts() {
        const scripts = (await this.files.list()).filter((file) => isProjectScript(file.path));
        return [...scripts].sort((left, right) => (right.modified?.getTime() ?? 0) - (left.modified?.getTime() ?? 0) ||
            left.path.localeCompare(right.path));
    }
    /** Open one script into a new tab named for its project; returns that tab's name. */
    async open(path) {
        const stored = await this.files.read(path);
        if (stored === null)
            throw new Error('The project script no longer exists.');
        return this.#build(path, stored);
    }
    /**
     * Open a script the person chose from disk, by its file name and text.
     *
     * A file chosen from the projects folder is that project's script, so its masks come with
     * it: it is recognised by a script there with the same file name and the same text. Any
     * other file opens as a script of its own, named by its header or its file name.
     */
    async openPicked(file, stored) {
        for (const candidate of await this.scripts()) {
            if (candidate.path.slice(candidate.path.lastIndexOf('/') + 1) !== file)
                continue;
            if ((await this.files.read(candidate.path)) === stored)
                return this.#build(candidate.path, stored);
        }
        return this.#build(file, stored);
    }
    /** Build `stored`, the text of the script at `path`, in a new tab. */
    async #build(path, stored) {
        // A script is stored with Windows line endings; a multi-line value holds plain newlines.
        const plain = stored.replace(/\r\n?/g, '\n');
        const owner = projectOwnerOf(path);
        const text = owner && this.masks ? await this.masks.bring(owner, plain) : plain;
        await this.workflows.checkScript(text);
        const name = uniqueTabName(projectNameOf(text, path), this.workflows.openNames());
        await this.workflows.openEmpty(name);
        try {
            await this.workflows.importScript(text);
        }
        catch (error) {
            // An empty tab under the project's name would save an empty script over the project.
            await this.workflows.discardOpened();
            throw error;
        }
        this.workflows.showCubes();
        return name;
    }
    /** Save the shown workflow as its project's script. */
    async save() {
        const name = this.#name();
        return this.#write(name, projectScriptPath(name));
    }
    /** Save the shown workflow under another file name inside its project's folder. */
    async saveAs(file) {
        const error = projectNameError(file);
        if (error)
            throw new Error(error);
        const name = this.#name();
        return this.#write(name, namedScriptPath(name, file));
    }
    /** The shown workflow's name, refused when it cannot name a project folder. */
    #name() {
        const name = this.workflows.activeName()?.trim() ?? '';
        const error = projectNameError(name);
        if (error)
            throw new Error(`The workflow's name cannot name a project. ${error}`);
        return name;
    }
    /** Render first, so a workflow that cannot be written leaves the existing script untouched. */
    async #write(name, path) {
        const rendered = await this.render(await this.workflows.serialize());
        const script = this.masks
            ? await this.masks.keep(name, rendered.source, rendered.maskFiles)
            : rendered.source;
        const existing = (await this.files.list()).find((file) => file.path === path);
        let kept = null;
        if (existing) {
            // The kept copy is named for when its file was created, as SugarSubstitute named it.
            const written = existing.created ?? existing.modified ?? this.now();
            for (let attempt = 0; attempt < MOVE_ATTEMPTS && kept === null; attempt += 1) {
                const destination = versionPath(path, written, attempt);
                const moved = await this.files.move(path, destination);
                if (moved === 'moved')
                    kept = destination;
                else if (moved === 'missing')
                    break;
            }
            if (kept === null && (await this.files.read(path)) !== null)
                throw new Error('The previous copy of the project script could not be kept.');
        }
        await this.files.write(path, projectDocument(name, script));
        this.workflows.markSaved();
        return { path, kept };
    }
}
