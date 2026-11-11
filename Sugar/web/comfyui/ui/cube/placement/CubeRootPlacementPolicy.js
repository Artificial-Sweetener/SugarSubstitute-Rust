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
/** Own the host-neutral rule that SugarCube wrappers belong only to a workflow root. */
/** Identify a rejected non-root SugarCube placement without parsing message text. */
export class CubeRootPlacementError extends Error {
    code = 'SUGARCUBE_ROOT_ONLY';
    /** Describe one rejected operation with stable remediation. */
    constructor(operation) {
        super(`SugarCubes can only be ${operation} on the top-level workflow. ` +
            'Open the top-level workflow and try again.');
        this.name = 'CubeRootPlacementError';
    }
}
/** Compare validated graph objects without depending on Comfy navigation or rendering state. */
export class CubeRootPlacementPolicy {
    #rootGraph;
    /** Retain the authoritative workflow root by object identity. */
    constructor(rootGraph) {
        this.#rootGraph = rootGraph;
    }
    /** Return whether a host target is the exact workflow root. */
    allows(targetGraph) {
        return targetGraph === this.#rootGraph;
    }
    /** Reject a missing or non-root target before a placement use case mutates state. */
    assertAllowed(targetGraph, operation = 'placed') {
        if (!this.allows(targetGraph))
            throw new CubeRootPlacementError(operation);
    }
    /** Return the authoritative root for host adapter identity comparisons. */
    rootGraph() {
        return this.#rootGraph;
    }
}
