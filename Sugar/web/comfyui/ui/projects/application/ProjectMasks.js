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
/** Carry a project's mask files between its folder and the place its Cubes load them from. */
import { maskInputName, quotesToken, replaceQuotedToken } from '../domain/ProjectScript.js';
/**
 * Keep a project's masks with the project.
 *
 * A project script names its masks by bare file name and keeps the files in the project's
 * `masks` folder. Mask loaders read from the host's input folder instead. Opening a project
 * puts each mask its script names into that folder and points the script at it; saving brings
 * every mask the workflow's loaders name back into the project and names it barely again.
 */
export class ProjectMasks {
    files;
    inputs;
    constructor(files, inputs) {
        this.files = files;
        this.inputs = inputs;
    }
    /** Return the script with each mask of `owner` it names made loadable. */
    async bring(owner, script) {
        const folder = `${owner}/masks/`;
        let text = script;
        for (const file of await this.files.list()) {
            const name = file.path.startsWith(folder) ? file.path.slice(folder.length) : '';
            if (!name || name.includes('/') || !name.toLowerCase().endsWith('.png'))
                continue;
            if (!quotesToken(text, name))
                continue;
            const bytes = await this.files.readBytes(file.path);
            if (!bytes)
                continue;
            const inputName = maskInputName(owner, name);
            await this.inputs.put(inputName, bytes);
            text = replaceQuotedToken(text, name, inputName);
        }
        return text;
    }
    /** Return the script with each named mask kept in `owner`'s folder under its project name. */
    async keep(owner, script, maskFiles) {
        let text = script;
        const taken = new Map();
        for (const mask of maskFiles) {
            if (!quotesToken(text, mask.file))
                continue;
            const bytes = await this.inputs.get(mask.file);
            // A mask the host no longer has keeps its name; the script still says what was used.
            if (!bytes)
                continue;
            let name = mask.name;
            // Two masks of one name from different places stay two files, numbered as before.
            for (let variant = 2; taken.has(name) && taken.get(name) !== mask.file; variant += 1)
                name = mask.name.replace(/(\.[^.]*)?$/u, `__v${String(variant).padStart(2, '0')}$1`);
            taken.set(name, mask.file);
            await this.files.writeBytes(`${owner}/masks/${name}`, bytes);
            text = replaceQuotedToken(text, mask.file, name);
        }
        return text;
    }
}
