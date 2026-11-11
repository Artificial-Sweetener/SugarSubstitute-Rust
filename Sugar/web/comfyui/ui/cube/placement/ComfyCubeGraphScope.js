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
/** Adapt Comfy's active-canvas graph to the root-only SugarCube placement policy. */
import { CubeRootPlacementPolicy } from './CubeRootPlacementPolicy.js';
/** Resolve current host graph state without leaking canvas details into application code. */
export class ComfyCubeGraphScope {
    #policy;
    #getCurrentGraph;
    /** Bind the validated workflow root and dynamic active graph resolver. */
    constructor(rootGraph, getCurrentGraph) {
        this.#policy = new CubeRootPlacementPolicy(rootGraph);
        this.#getCurrentGraph = getCurrentGraph;
    }
    /** Return whether Comfy currently targets the workflow root. */
    isCurrentRoot() {
        return this.#policy.allows(this.#getCurrentGraph());
    }
    /** Reject the current target before any Sugar-owned placement side effect. */
    assertCurrentRoot(operation = 'placed') {
        this.#policy.assertAllowed(this.#getCurrentGraph(), operation);
    }
    /** Apply the same invariant to a host-supplied target graph. */
    assertTargetRoot(targetGraph, operation = 'placed') {
        this.#policy.assertAllowed(targetGraph, operation);
    }
    /** Return the policy used by host mutation guards. */
    policy() {
        return this.#policy;
    }
}
