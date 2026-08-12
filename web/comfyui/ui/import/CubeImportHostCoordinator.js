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
/** Coordinate prepared imports and Cube browser placement host adapters. */
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import { coerceVec2, readVector2 } from '../graph/VectorUtils.js';
import { computePayloadBounds, drawGhostRect, getPlacementGroupLabel, resolvePreviewRect, } from '../overlays/PlacementHelpers.js';
import { ComfyCanvasDropOriginAdapter, } from './ComfyCanvasDropOriginAdapter.js';
import { CubeImportCommandService } from './CubeImportCommandService.js';
import { CubeImportOutcomeReporter } from './CubeImportOutcomeReporter.js';
import { CubePreparedImportService } from './CubePreparedImportService.js';
/** Own Cube import commands, feedback, and placement adapter wiring. */
export class CubeImportHostCoordinator {
    #ui;
    #app;
    #feedback;
    #getRuntime;
    #outcomes;
    #preparedImports;
    #commands;
    #dropOrigin;
    constructor(options) {
        this.#ui = options.ui;
        this.#app = options.app;
        this.#feedback = options.feedback;
        this.#getRuntime = options.getRuntime;
        const { adapter, overlayManager } = this.#ui;
        this.#dropOrigin = new ComfyCanvasDropOriginAdapter({
            getCanvas: () => adapter.getCanvas(),
            logger: options.logger,
        });
        this.#preparedImports = new CubePreparedImportService({
            getGraph: () => this.#app?.graph,
            getLiteGraph: () => adapter.getLiteGraph?.(),
            getNodeRenderer: () => adapter.getNodeRenderer?.(),
            getRuntime: this.#getRuntime,
            assertRootPlacement: () => this.#getRuntime().graphScope.assertCurrentRoot('imported'),
            readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
        });
        this.#outcomes = new CubeImportOutcomeReporter({
            focusImportedCube: (result) => this.#focusImportedCube(result),
            pushToast: (severity, summary, detail) => this.#feedback.pushToast(severity, summary, detail),
        });
        this.#commands = new CubeImportCommandService({
            api: this.#ui.api,
            applyPreparedImport: (payload, importOptions) => this.apply(payload, importOptions),
            computeDropOrigin: () => this.computeDropOrigin(),
            reportOutcome: (defaultAlias, warnings, result, payload) => this.#outcomes.report(defaultAlias, warnings, result, payload),
            persistLastCubeId: (cubeId) => this.#feedback.persistLastCubeId(cubeId),
            pushToast: (severity, summary, detail) => this.#feedback.pushToast(severity, summary, detail),
            readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
        });
        this.#ui.cubeBrowser.configure({
            actions: {
                computeDropOrigin: () => this.computeDropOrigin(),
                importCubeByName: (cubeId, importOptions) => this.#commands.importCurrent(cubeId, importOptions),
                importCubeRevision: (cubeId, revisionRef, importOptions) => this.#commands.importRevision(cubeId, revisionRef, importOptions),
                onCubesUpdated: (cubes) => this.#ui.dirtyManager.updateKnownCubes(cubes),
                openConfirmDialog: (dialogOptions) => this.#ui.confirmDialog.open(dialogOptions),
                promoteCube: (cube) => this.#ui.promotionService.promote(cube),
                reconcileCubeIdentity: (identity) => this.#ui.identityReconciler.reconcile(identity),
                startCubePlacement: (cubeId, placementOptions) => overlayManager.placement.start(cubeId, placementOptions),
            },
            helpers: {
                coerceVec2,
                computePayloadBounds: (entries, context) => computePayloadBounds(entries, context, adapter.getLiteGraph?.()),
                drawGhostRect,
                getPlacementGroupLabel: (defaultAlias, group) => getPlacementGroupLabel(defaultAlias, group, getGroupSugarcubes),
                readVector2,
                resolvePreviewRect: (entry, position, size, context) => resolvePreviewRect(entry, position, size, context, adapter.getLiteGraph?.()),
            },
            placement: {
                commit: () => overlayManager.placement.commit(),
                computeOriginFromEvent: (event) => overlayManager.placement.computeOriginFromEvent(event),
                getState: () => overlayManager.placement.getState(),
                isPointerOverCanvas: (event) => overlayManager.placement.isPointerOverCanvas(event),
                setCommitInProgress: (value) => overlayManager.placement.setCommitInProgress(value),
                setDirty: () => overlayManager.placement.setDirty(),
                setOrigin: (origin) => overlayManager.placement.setOrigin(origin),
                start: (cubeId, placementOptions) => overlayManager.placement.start(cubeId, {
                    closeBrowser: placementOptions.closeBrowser,
                    ...(placementOptions.defaultAlias
                        ? { defaultAlias: placementOptions.defaultAlias }
                        : {}),
                }),
                stop: (reason) => overlayManager.placement.stop(reason),
            },
        });
    }
    /** Apply one prepared import through the graph-owned importer. */
    async apply(payload, options = {}) {
        return this.#preparedImports.apply(payload, options);
    }
    /** Report one import result through focus and host feedback owners. */
    report(defaultAlias, backendWarnings, importResult, payload, options = {}) {
        this.#outcomes.report(defaultAlias, backendWarnings, importResult, payload, options);
    }
    /** Return the current graph-space drop origin. */
    computeDropOrigin() {
        return this.#dropOrigin.compute();
    }
    /** Focus the canvas on the imported Cube bounds when available. */
    #focusImportedCube(result) {
        const canvas = this.#ui.adapter.getCanvas();
        const bounds = result.bounds;
        if (!canvas || !bounds)
            return;
        const rectangle = [
            bounds.minX,
            bounds.minY,
            bounds.maxX - bounds.minX,
            bounds.maxY - bounds.minY,
        ];
        const redraw = () => canvas.setDirty?.(true, true);
        if (typeof canvas.ds?.animateToBounds === 'function') {
            canvas.ds.animateToBounds(rectangle, redraw);
        }
        else {
            canvas.ds?.fitToBounds?.(rectangle);
            redraw();
        }
    }
}
