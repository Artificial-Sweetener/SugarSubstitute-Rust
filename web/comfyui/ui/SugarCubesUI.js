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
 * Own the SugarCubes UI orchestration layer in `frontend/comfyui/ui/SugarCubesUI.js`.
 */
import { ComfyAdapter } from './core/ComfyAdapter.js';
import { EventBus } from './core/EventBus.js';
import { Scheduler } from './core/Scheduler.js';
import { ToastService } from './core/ToastService.js';
import { StorageService } from './core/StorageService.js';
import { CubeLibraryApi } from './core/CubeLibraryApi.js';
import { OverlayManager } from './overlays/OverlayManager.js';
import { CubeBrowserController } from './browser/CubeBrowserController.js';
import { ConfirmDialog } from './dialogs/ConfirmDialog.js';
import { ModalService } from './dialogs/ModalService.js';
import { VersionDialog } from './dialogs/VersionDialog.js';
import { InstanceManager } from './graph/InstanceManager.js';
import { DirtyManager } from './graph/DirtyManager.js';
import { CubeSaveService } from './save/CubeSaveService.js';
import { CubeLayoutService } from './layout/CubeLayoutService.js';
import { CubeContainmentService } from './layout/CubeContainmentService.js';
import { CubeCollisionService } from './layout/CubeCollisionService.js';
import { CubeBoundsReconciler } from './layout/CubeBoundsReconciler.js';
import { FlavorService } from './flavors/FlavorService.js';
import { CubeDefinitionStore } from './graph/CubeDefinitionStore.js';
import { CubeSaveReconciler } from './save/CubeSaveReconciler.js';
import { CubeCreationService } from './create/CubeCreationService.js';
import { CubePackService } from './packs/CubePackService.js';
import { CubeIdentityReconciler } from './graph/CubeIdentityReconciler.js';
import { CubePromotionService } from './promotion/CubePromotionService.js';
/**
 * Coordinate sugar cubes ui behavior for the SugarCubes UI.
 */
export class SugarCubesUI {
    adapter;
    events;
    scheduler;
    storage;
    api;
    dialogs;
    toast;
    confirmDialog;
    versionDialog;
    cubeBrowser;
    definitionStore;
    instanceManager;
    dirtyManager;
    flavorService;
    saveReconciler;
    packService;
    identityReconciler;
    promotionService;
    cubeSave;
    cubeCreation;
    layoutService;
    containmentService;
    collisionService;
    boundsReconciler;
    overlayManager;
    _setupDone;
    constructor(options = {}) {
        this.adapter = options.adapter || new ComfyAdapter(options.adapterOptions);
        this.events = options.events || new EventBus();
        this.scheduler = options.scheduler || new Scheduler(this.adapter);
        this.storage = options.storage || new StorageService(this.adapter);
        this.api = options.cubeApi || new CubeLibraryApi(this.adapter);
        this.dialogs = options.dialogs || new ModalService({ adapter: this.adapter });
        this.toast = options.toast || new ToastService(this.adapter, { dialogs: this.dialogs });
        this.confirmDialog = this.dialogs.confirmDialog || new ConfirmDialog({ adapter: this.adapter });
        this.versionDialog = new VersionDialog({ adapter: this.adapter, storage: this.storage });
        this._setupDone = false;
        this.cubeBrowser = new CubeBrowserController({
            adapter: this.adapter,
            api: this.api,
            events: this.events,
            storage: this.storage,
            toast: this.toast,
            scheduler: this.scheduler,
        });
        this.definitionStore = new CubeDefinitionStore({
            api: this.api,
            logger: this.adapter?.getConsole?.(),
            onUpdate: (definitionKey, entry) => this.handleDefinitionUpdate(definitionKey, entry),
        });
        this.instanceManager = new InstanceManager({
            adapter: this.adapter,
            events: this.events,
            scheduler: this.scheduler,
            requestDirtyRefresh: (opts) => this.dirtyManager.requestRefresh(opts),
        });
        this.dirtyManager = new DirtyManager({
            adapter: this.adapter,
            events: this.events,
            scheduler: this.scheduler,
            cubeBrowser: this.cubeBrowser,
            definitionStore: this.definitionStore,
        });
        this.flavorService = new FlavorService({
            adapter: this.adapter,
            dialogs: this.dialogs,
            events: this.events,
            storage: this.storage,
            toast: this.toast,
            api: this.api,
            dirtyManager: this.dirtyManager,
            cubeBrowser: this.cubeBrowser,
        });
        this.saveReconciler = new CubeSaveReconciler({
            definitionStore: this.definitionStore,
            instanceManager: this.instanceManager,
            flavorService: this.flavorService,
            dirtyManager: this.dirtyManager,
        });
        this.packService = new CubePackService({
            api: this.api,
            dialogs: this.dialogs,
            toast: this.toast,
        });
        this.identityReconciler = new CubeIdentityReconciler({
            adapter: this.adapter,
            instanceManager: this.instanceManager,
            dirtyManager: this.dirtyManager,
            definitionStore: this.definitionStore,
        });
        this.promotionService = new CubePromotionService({
            api: this.api,
            dialogs: this.dialogs,
            toast: this.toast,
            packService: this.packService,
            identityReconciler: this.identityReconciler,
            cubeBrowser: this.cubeBrowser,
        });
        this.cubeSave = new CubeSaveService({
            adapter: this.adapter,
            api: this.api,
            toast: this.toast,
            instanceManager: this.instanceManager,
            dirtyManager: this.dirtyManager,
            cubeBrowser: this.cubeBrowser,
            versionDialog: this.versionDialog,
            dialogs: this.dialogs,
            saveReconciler: this.saveReconciler,
        });
        this.cubeCreation = new CubeCreationService({
            adapter: this.adapter,
            api: this.api,
            toast: this.toast,
            instanceManager: this.instanceManager,
            cubeBrowser: this.cubeBrowser,
            dialogs: this.dialogs,
            saveReconciler: this.saveReconciler,
        });
        this.layoutService = new CubeLayoutService({
            adapter: this.adapter,
            instanceManager: this.instanceManager,
            dirtyManager: this.dirtyManager,
        });
        this.containmentService = new CubeContainmentService();
        this.collisionService = new CubeCollisionService();
        this.boundsReconciler = new CubeBoundsReconciler();
        this.overlayManager = new OverlayManager({
            adapter: this.adapter,
            events: this.events,
            scheduler: this.scheduler,
            storage: this.storage,
            api: this.adapter?.getApi?.(),
            cubeApi: this.api,
            cubeBrowser: this.cubeBrowser,
            saveService: this.cubeSave,
            flavorService: this.flavorService,
            toast: this.toast,
            ...(options.applyPreparedImport ? { applyPreparedImport: options.applyPreparedImport } : {}),
            ...(options.reportImportOutcome ? { reportImportOutcome: options.reportImportOutcome } : {}),
            ...(options.buildShiftedPlacementPayload
                ? { buildShiftedPlacementPayload: options.buildShiftedPlacementPayload }
                : {}),
            requestDirtyRefresh: (opts) => this.dirtyManager.requestRefresh(opts),
            layoutService: this.layoutService,
            containmentService: this.containmentService,
            collisionService: this.collisionService,
            boundsReconciler: this.boundsReconciler,
        });
    }
    /** Publish definition updates to consumers without assigning them cache ownership. */
    handleDefinitionUpdate(definitionKey, entry) {
        const graph = this.adapter?.getApp?.()?.graph || null;
        if (entry?.status === 'ready' && entry?.payload) {
            this.events?.emit?.('cube:definition:loaded', {
                cubeId: entry.cubeId,
                definitionKey,
                entry,
                graph,
            });
        }
        if (graph) {
            this.dirtyManager?.requestRefresh?.({ graph, reason: 'definition-update' });
        }
    }
    async setup() {
        if (this._setupDone) {
            return;
        }
        await this.cubeBrowser.setup();
        this.instanceManager.setup();
        this.dirtyManager.setup();
        await this.flavorService.setup();
        this.events.on('cube:instances:refresh', (options) => {
            if (!options || typeof options !== 'object') {
                return;
            }
            this.instanceManager.scheduleRefresh(options);
        });
        this.overlayManager.setup();
        this._setupDone = true;
    }
    dispose() {
        this.overlayManager.dispose();
        this.cubeBrowser.dispose();
        this.dirtyManager.dispose();
        this.flavorService.dispose();
    }
    async listCubes() {
        const { data } = await this.api.list();
        return data;
    }
    async previewCube(cubeId) {
        const { data } = await this.api.preview(cubeId);
        return data;
    }
    openLibrary(options = {}) {
        return this.cubeBrowser.open(options);
    }
    scheduleCubeInstanceRefresh(options = {}) {
        this.instanceManager.scheduleRefresh(options);
    }
    scheduleCubeDirtyRefresh(options = {}) {
        this.dirtyManager.requestRefresh(options);
    }
}
