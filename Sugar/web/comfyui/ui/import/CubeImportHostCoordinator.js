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
/** Coordinate native workflow imports and SugarScript host adapters. */
import { isRecord } from '../types/common.js';
import { ComfyCanvasDropOriginAdapter, } from './ComfyCanvasDropOriginAdapter.js';
import { SugarScriptAuthoringApi } from '../sugarscript/SugarScriptAuthoringApi.js';
import { SugarScriptFileDropAdapter } from '../sugarscript/SugarScriptFileDropAdapter.js';
import { WorkflowFileDragAcceptor } from './WorkflowFileDragAcceptor.js';
import { SugarScriptImportService } from '../sugarscript/SugarScriptImportService.js';
import { isSugarScriptFileHost, SugarScriptImageFileHostAdapter, } from '../sugarscript/SugarScriptImageFileHostAdapter.js';
import { SugarScriptWorkflowArtifactCompanionService } from '../sugarscript/SugarScriptWorkflowArtifactCompanionService.js';
import { NativeWorkflowMaterializer } from '../workflow/NativeWorkflowMaterializer.js';
import { LegacyWorkflowAuthoringApi } from '../workflow/LegacyWorkflowAuthoringApi.js';
import { LegacyWorkflowImportService } from '../workflow/LegacyWorkflowImportService.js';
import { recordExactIntegers } from '../workflow/ExactIntegerRelations.js';
import { fixImportedSeeds } from '../workflow/ImportedSeedModes.js';
import { carryScriptComposition, markUnsavedOutputs, readUnsavedOutputInstances, } from '../workflow/ImportedScriptState.js';
/** Own native workflow import entrypoints and their file adapters. */
export class CubeImportHostCoordinator {
    #ui;
    #app;
    #feedback;
    #getRuntime;
    #relations;
    #dropOrigin;
    #sugarScriptApi;
    #legacyWorkflowApi;
    #sugarScriptFiles;
    #sugarScriptImages;
    constructor(options) {
        this.#ui = options.ui;
        this.#app = options.app;
        this.#feedback = options.feedback;
        this.#getRuntime = options.getRuntime;
        this.#relations = options.relations;
        const { adapter } = this.#ui;
        this.#dropOrigin = new ComfyCanvasDropOriginAdapter({
            getCanvas: () => adapter.getCanvas(),
            logger: options.logger,
        });
        this.#sugarScriptApi = new SugarScriptAuthoringApi(this.#ui.api);
        this.#legacyWorkflowApi = new LegacyWorkflowAuthoringApi(this.#ui.api);
        const document = adapter.getDocument();
        this.#sugarScriptFiles = document
            ? new SugarScriptFileDropAdapter({
                document,
                importSource: (source) => this.importSugarScript(source),
                feedback: this.#ui.toast,
                readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
            })
            : null;
        this.#sugarScriptFiles?.setup();
        // After the SugarScript owner, so a `.sugar` file is still answered by it first.
        if (document)
            new WorkflowFileDragAcceptor(document).setup();
        this.#sugarScriptImages = isSugarScriptFileHost(this.#app)
            ? new SugarScriptImageFileHostAdapter({
                host: this.#app,
                importSource: (source) => this.importSugarScript(source),
                importWorkflow: (workflow, source) => this.openLegacyWorkflow(workflow, source),
                validateWorkflow: (workflow) => this.#legacyWorkflowApi.compile(workflow),
                retainWorkflowSource: (source) => new SugarScriptWorkflowArtifactCompanionService(this.#sugarScriptApi, () => this.#app?.graph ?? null).retain(source),
                feedback: this.#ui.toast,
                readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
            })
            : null;
        this.#sugarScriptImages?.setup();
    }
    /** Return the current graph-space drop origin. */
    computeDropOrigin() {
        return this.#dropOrigin.compute();
    }
    /** Compile source and atomically create a self-contained native Cube workflow. */
    async importSugarScript(source, origin = this.computeDropOrigin()) {
        const runtime = this.#getRuntime();
        runtime.graphScope.assertCurrentRoot('SugarScript import');
        const graph = this.#app?.graph;
        if (!graph)
            throw new Error('Comfy root graph is unavailable.');
        return new SugarScriptImportService(this.#sugarScriptApi, this.#nativeWorkflowMaterializer(runtime, graph), {
            recordExact: (fields) => recordExactIntegers(graph, fields),
            adoptNodeLinks: (instanceIds, links) => this.#relations().adoptNodeLinks(instanceIds, links),
            adoptRelations: (instanceIds, script) => this.#relations().adoptScriptRelations(instanceIds, script),
            carry: (workflow) => {
                carryScriptComposition(graph, workflow);
                markUnsavedOutputs(graph, readUnsavedOutputInstances(workflow));
            },
        }).import(source, origin);
    }
    /** Reconcile and atomically import a persisted group-era Cube workflow. */
    async importLegacyWorkflow(workflow, origin = this.computeDropOrigin(), source) {
        return this.#legacyWorkflows().import(workflow, origin, source);
    }
    /**
     * Open a workflow a file carries in a workflow of its own, as Comfy opens any workflow file.
     * The file's Cubes keep their identities, so adding them to the workflow already open would
     * merge two workflows, or fail where both hold the same Cube.
     */
    async openLegacyWorkflow(workflow, source) {
        return this.#legacyWorkflows().open(workflow, source, async () => {
            await this.#openBlankWorkflow();
            return this.computeDropOrigin();
        });
    }
    /** Ask Comfy for a new blank workflow and wait until it is the one on screen. */
    async #openBlankWorkflow() {
        const app = this.#app;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        const command = manager && isRecord(manager.command) ? manager.command : null;
        const execute = command?.execute;
        if (!command || typeof execute !== 'function')
            throw new Error('Comfy cannot open a new workflow for this file.');
        await Reflect.apply(execute, command, ['Comfy.NewBlankWorkflow']);
    }
    /** Bind the persisted-workflow import to the current root graph. */
    #legacyWorkflows() {
        const runtime = this.#getRuntime();
        runtime.graphScope.assertCurrentRoot('persisted Cube workflow import');
        const graph = this.#app?.graph;
        if (!graph)
            throw new Error('Comfy root graph is unavailable.');
        return new LegacyWorkflowImportService(this.#legacyWorkflowApi, this.#nativeWorkflowMaterializer(runtime, graph), (fields) => recordExactIntegers(graph, fields), (instanceIds) => {
            fixImportedSeeds(graph, instanceIds);
            this.#relations().holdSeeds(instanceIds);
        }, (instanceIds, links) => this.#relations().adoptNodeLinks(instanceIds, links));
    }
    /** Build the shared native workflow mutation owner for one root graph. */
    #nativeWorkflowMaterializer(runtime, graph) {
        return new NativeWorkflowMaterializer(runtime.placement, graph, {
            register: (payload) => runtime.registerSubgraphs(payload),
            discard: (ids) => runtime.discardSubgraphs(ids),
        });
    }
}
