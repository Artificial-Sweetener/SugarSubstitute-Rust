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
/** Reuse canonical analyses of identical workflow content across runtimes and workflow switches. */
import { contentDigest } from '../../core/ContentDigest.js';
const DEFAULT_CAPACITY = 8;
/**
 * Serve repeated analysis of the same serialized workflow from one in-flight or settled result.
 *
 * Analysis is a pure function of the submitted workflow, so switching back to an unchanged tab,
 * or re-snapshotting a workflow whose content did not change, needs no request. Mutating
 * operations always reach the gateway. Failures are never retained.
 */
export class CubeGraphAnalysisCache {
    #gateway;
    #capacity;
    #analyses = new Map();
    /** Wrap the authoritative gateway for the page lifetime. */
    constructor(gateway, capacity = DEFAULT_CAPACITY) {
        this.#gateway = gateway;
        this.#capacity = Math.max(1, capacity);
    }
    /** Analyze one snapshot, sharing results for identical serialized content. */
    analyze(snapshot) {
        const key = contentDigest(snapshot.serializedWorkflow);
        const cached = this.#analyses.get(key);
        if (cached) {
            this.#analyses.delete(key);
            this.#analyses.set(key, cached);
            return cached;
        }
        const analysis = this.#gateway.analyze(snapshot);
        this.#analyses.set(key, analysis);
        analysis.catch(() => {
            if (this.#analyses.get(key) === analysis)
                this.#analyses.delete(key);
        });
        while (this.#analyses.size > this.#capacity) {
            const oldest = this.#analyses.keys().next().value;
            if (oldest === undefined)
                break;
            this.#analyses.delete(oldest);
        }
        return analysis;
    }
    /** Submit every reorder to the authoritative gateway. */
    reorder(snapshot, segmentInstanceIds, orderedInstanceIds) {
        return this.#gateway.reorder(snapshot, segmentInstanceIds, orderedInstanceIds);
    }
    /** Forget every analysis after the catalog that resolves workflow content changes. */
    invalidate() {
        this.#analyses.clear();
    }
}
