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
/** Coordinate persisted-workflow reconciliation and native materialization. */
/** Keep legacy workflow loading behind exact-version reconciliation. */
export class LegacyWorkflowImportService {
    #api;
    #materializer;
    #recordExact;
    #fixSeeds;
    /** Bind transport, graph mutation and the workflow's record of exact whole numbers. */
    constructor(api, materializer, recordExact, fixSeeds) {
        this.#api = api;
        this.#materializer = materializer;
        this.#recordExact = recordExact;
        this.#fixSeeds = fixSeeds;
    }
    /**
     * Reconcile and import one workflow without queueing execution. Digits of whole numbers this
     * page cannot hold are recorded on the workflow only once its Cubes exist.
     */
    async import(workflow, origin, source) {
        return this.#materialize(await this.#api.compile(workflow, source), origin, source);
    }
    /**
     * Open one workflow from a file as a workflow of its own. It is reconciled first, so a file
     * that cannot be imported opens nothing; `place` then opens the workflow that will hold its
     * Cubes and returns where they go. The Cubes keep the identities the file gave them, which a
     * workflow already open may also hold, so they never join it.
     */
    async open(workflow, source, place) {
        const response = await this.#api.compile(workflow, source);
        if (!response.plan)
            throw new Error('Persisted Cube workflow produced no native plan.');
        return this.#materialize(response, await place(), source);
    }
    /** Create the reconciled workflow's Cubes and record what only they can hold. */
    #materialize(response, origin, source) {
        if (!response.plan)
            throw new Error('Persisted Cube workflow produced no native plan.');
        const result = this.#materializer.materializeWorkflow(response.plan, origin);
        // A workflow sent as its saved text was carried as Cube documents, which record no seed
        // mode: its seeds stay as the image recorded them.
        if (source !== undefined)
            this.#fixSeeds(result.instanceIds);
        if (response.exactIntegers.length)
            this.#recordExact(response.exactIntegers);
        return result;
    }
}
