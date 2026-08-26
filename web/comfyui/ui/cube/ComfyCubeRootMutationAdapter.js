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
/** Adapt Comfy's root graph to the shared native Cube mutation contract. */
import { isRecord } from '../types/common.js';
/** Centralize validated root insertion, removal, node lookup, and link lookup. */
export class ComfyCubeRootMutationAdapter {
    #add;
    #remove;
    #getNodeById;
    #getLink;
    /** Capture bound host methods once so every mutation owner sees one graph. */
    constructor(graph) {
        this.#add = requireBoundMethod(graph, 'add');
        this.#remove = requireBoundMethod(graph, 'remove');
        this.#getNodeById = requireBoundMethod(graph, 'getNodeById');
        this.#getLink = requireBoundMethod(graph, 'getLink');
    }
    /** Insert one native Cube node into the authoritative root graph. */
    add(node) {
        this.#add(node);
    }
    /** Remove one native Cube node from the authoritative root graph. */
    remove(node) {
        this.#remove(node);
    }
    /** Return a root node without imposing Cube identity on the lookup result. */
    getNodeById(id) {
        return this.#getNodeById(id);
    }
    /** Return one normalized Comfy link record. */
    getLink(id) {
        const value = this.#getLink(id);
        return isRecord(value) ? value : null;
    }
}
/** Require and bind one dynamic Comfy graph method. */
function requireBoundMethod(owner, name) {
    const value = owner[name];
    if (typeof value !== 'function')
        throw new TypeError(`LGraph.${name} is unavailable.`);
    return (...args) => Reflect.apply(value, owner, args);
}
