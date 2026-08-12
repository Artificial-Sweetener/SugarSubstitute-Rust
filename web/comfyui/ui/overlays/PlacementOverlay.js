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
/**
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/PlacementOverlay.js`.
 */
import { PlacementPreviewPainter } from './PlacementPreviewPainter.js';
import { PlacementPointerController } from './PlacementPointerController.js';
import { PlacementDomLifecycle } from './PlacementDomLifecycle.js';
import { PlacementCommandCoordinator } from './PlacementCommandCoordinator.js';
/**
 * Coordinate placement overlay behavior for the SugarCubes UI.
 */
export class PlacementOverlay {
    events;
    logger;
    state;
    previewPainter;
    pointer;
    domLifecycle;
    commands;
    constructor({ adapter = null, events = null, scheduler = null, cubeApi = null, cubeBrowser = null, toast = null, applyPreparedImport, reportImportOutcome, buildShiftedPlacementPayload, } = {}) {
        this.events = events;
        this.logger = adapter?.getConsole?.() || null;
        this.state = {
            active: false,
            cubeId: null,
            defaultAlias: null,
            payload: null,
            baseOrigin: [0, 0],
            origin: [0, 0],
            canvasElement: null,
            handlers: null,
            commitInProgress: false,
            overlayElement: null,
            overlayRafId: null,
            cubeVersion: '',
            cubeRevisionRef: 'WORKTREE',
        };
        this.previewPainter = new PlacementPreviewPainter(adapter, this.state);
        this.pointer = new PlacementPointerController({
            adapter,
            state: this.state,
            logger: this.logger,
            commit: () => this.commit(),
            stop: (reason) => this.stop(reason),
        });
        this.domLifecycle = new PlacementDomLifecycle({
            adapter,
            scheduler,
            state: this.state,
            pointer: this.pointer,
            setDirty: () => this.setDirty(),
            stop: (reason) => this.stop(reason),
        });
        this.commands = new PlacementCommandCoordinator({
            adapter,
            cubeApi,
            cubeBrowser,
            toast,
            state: this.state,
            applyPreparedImport,
            reportImportOutcome,
            buildShiftedPlacementPayload,
            stop: (reason) => this.stop(reason),
            computeDropOrigin: () => this.computeDropOrigin(),
            installPlacementHandlers: () => this.installPlacementHandlers(),
            setPlacementSidebarVisibility: (active) => this.setPlacementSidebarVisibility(active),
            setDirty: () => this.setDirty(),
        });
    }
    getState() {
        return this.state;
    }
    setOrigin(origin) {
        this.pointer.setOrigin(origin);
    }
    setCommitInProgress(value) {
        this.pointer.setCommitInProgress(value);
    }
    setDirty() {
        this.pointer.setDirty();
    }
    computeOriginFromEvent(event) {
        return this.pointer.computeOriginFromEvent(event);
    }
    isPointerOverCanvas(event) {
        return this.pointer.isPointerOverCanvas(event);
    }
    convertCanvasPoint(canvasInstance, point) {
        return this.pointer.convertCanvasPoint(canvasInstance, point);
    }
    computeDropOrigin() {
        return this.pointer.computeDropOrigin();
    }
    resolvePlacementOrigin(canvasInstance, event) {
        return this.pointer.resolvePlacementOrigin(canvasInstance, event);
    }
    handlePlacementMouseDown(event, canvasInstance, overlayElement = null) {
        return this.pointer.handlePlacementMouseDown(event, canvasInstance, overlayElement);
    }
    updatePlacementOverlayBounds() {
        this.domLifecycle.updatePlacementOverlayBounds();
    }
    stopPlacementOverlayLoop() {
        this.domLifecycle.stopPlacementOverlayLoop();
    }
    startPlacementOverlayLoop() {
        this.domLifecycle.startPlacementOverlayLoop();
    }
    ensurePlacementOverlay() {
        return this.domLifecycle.ensurePlacementOverlay();
    }
    showPlacementOverlay() {
        this.domLifecycle.showPlacementOverlay();
    }
    hidePlacementOverlay() {
        this.domLifecycle.hidePlacementOverlay();
    }
    removePlacementHandlers() {
        this.domLifecycle.removePlacementHandlers();
    }
    stop(reason = null) {
        this.domLifecycle.stop(reason);
    }
    setPlacementSidebarVisibility(isActive) {
        this.domLifecycle.setPlacementSidebarVisibility(isActive);
    }
    installPlacementHandlers() {
        return this.domLifecycle.installPlacementHandlers();
    }
    async start(cubeId, options = {}) {
        await this.commands.start(cubeId, options);
    }
    async commit() {
        await this.commands.commit();
    }
    async applyPreparedPayload(payload, options) {
        return this.commands.applyPreparedPayload(payload, options);
    }
    render(ctx, canvasInstance) {
        this.previewPainter.render(ctx, canvasInstance);
    }
}
