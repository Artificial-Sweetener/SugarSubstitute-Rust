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
 * Own Cube revision loading and placement commit orchestration.
 */
import { isCurrentRevisionRef, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
import { planPlacementGeometry } from '../geometry/PlacementGeometryPlanner.js';
import { resolveRendererGeometryPolicy } from '../geometry/RendererGeometryPolicy.js';
import { readImportPayload } from '../import/PlacementPayload.js';
import { readVector2 } from '../graph/VectorUtils.js';
/** Coordinate preview loading and prepared import commit. */
export class PlacementCommandCoordinator {
    options;
    adapter;
    cubeApi;
    cubeBrowser;
    toast;
    state;
    applyPreparedImport;
    reportImportOutcome;
    buildShiftedPlacementPayload;
    constructor(options) {
        this.options = options;
        this.adapter = options.adapter;
        this.cubeApi = options.cubeApi;
        this.cubeBrowser = options.cubeBrowser;
        this.toast = options.toast;
        this.state = options.state;
        this.applyPreparedImport = options.applyPreparedImport;
        this.reportImportOutcome = options.reportImportOutcome;
        this.buildShiftedPlacementPayload = options.buildShiftedPlacementPayload;
    }
    async start(cubeId, options = {}) {
        const trimmed = typeof cubeId === 'string' ? cubeId.trim() : '';
        if (!trimmed) {
            this.toast?.push?.('warn', 'Cube required', 'Select a cube before placing.');
            return;
        }
        const displayName = typeof options.defaultAlias === 'string' && options.defaultAlias.trim()
            ? options.defaultAlias.trim()
            : trimmed;
        if (!this.cubeApi) {
            this.toast?.push?.('error', 'Placement unavailable', 'Cube library API is unavailable.');
            return;
        }
        this.options.stop();
        this.cubeBrowser?.setBusy?.(true);
        try {
            const revisionRef = normalizeRevisionRef(options.revisionRef);
            const loader = isCurrentRevisionRef(revisionRef)
                ? this.cubeApi.load.bind(this.cubeApi)
                : this.cubeApi.loadRevision.bind(this.cubeApi);
            const body = isCurrentRevisionRef(revisionRef)
                ? { cube_id: trimmed, origin: { x: 0, y: 0 } }
                : { cube_id: trimmed, revision_ref: revisionRef, origin: { x: 0, y: 0 } };
            const { response, data } = await loader(JSON.stringify(body), {
                headers: { 'Content-Type': 'application/json' },
            });
            if (!response.ok || data?.error) {
                const errorPayload = isRecord(data.error) ? data.error : {};
                const message = typeof errorPayload.message === 'string'
                    ? errorPayload.message
                    : response.statusText || 'Placement preview failed';
                const detail = typeof errorPayload.detail === 'string' && errorPayload.detail ? errorPayload.detail : '';
                this.toast?.push?.('error', message, detail);
                return;
            }
            this.state.active = true;
            this.state.cubeId = trimmed;
            this.state.defaultAlias = displayName;
            const importPayload = readImportPayload(data);
            if (!importPayload) {
                this.toast?.push?.('error', 'Placement preview failed', 'Importer payload missing.');
                return;
            }
            const placementPayload = planPlacementGeometry(importPayload, resolveRendererGeometryPolicy(this.adapter?.getLiteGraph?.(), this.adapter?.getNodeRenderer?.()));
            this.state.payload = placementPayload;
            this.state.cubeVersion =
                typeof options.version === 'string' && options.version.trim()
                    ? options.version.trim()
                    : typeof placementPayload.cube?.version === 'string'
                        ? placementPayload.cube.version.trim()
                        : '';
            this.state.cubeRevisionRef = revisionRef;
            this.state.baseOrigin = readVector2(placementPayload.layout?.origin, 0, 0);
            this.state.origin = Array.isArray(options.origin)
                ? options.origin
                : this.options.computeDropOrigin();
            const handlersInstalled = this.options.installPlacementHandlers();
            if (!handlersInstalled) {
                this.options.stop('Placement unavailable: canvas missing.');
                return;
            }
            this.options.setPlacementSidebarVisibility(true);
            if (options.closeBrowser) {
                this.cubeBrowser?.close?.();
                this.toast?.push?.('info', 'Place SugarCube', 'Click on the canvas to place it. Press Esc to cancel.');
            }
            this.options.setDirty();
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.toast?.push?.('error', 'Placement failed', message);
        }
        finally {
            this.cubeBrowser?.setBusy?.(false);
        }
    }
    async commit() {
        if (!this.state.active || !this.state.cubeId) {
            return;
        }
        const defaultAlias = this.state.defaultAlias || this.state.cubeId;
        const baseOrigin = this.state.baseOrigin;
        const targetOrigin = this.state.origin;
        const shift = [targetOrigin[0] - baseOrigin[0], targetOrigin[1] - baseOrigin[1]];
        const payload = this.buildShiftedPlacementPayload?.(this.state.payload, shift, targetOrigin);
        this.options.stop();
        if (!payload) {
            return;
        }
        this.cubeBrowser?.setBusy?.(true);
        const result = await this.applyPreparedPayload(payload, {
            instanceAlias: defaultAlias,
            dropOrigin: targetOrigin,
        });
        const backendWarnings = Array.isArray(payload?.warnings)
            ? payload.warnings.filter(Boolean)
            : [];
        this.reportImportOutcome?.(defaultAlias, backendWarnings, result, payload, { focus: false });
        this.cubeBrowser?.setBusy?.(false);
        if (result?.success) {
            this.cubeBrowser?.close?.();
        }
    }
    /** Apply a prepared import through the configured application boundary. */
    async applyPreparedPayload(payload, options) {
        return (await this.applyPreparedImport?.(payload, options)) ?? null;
    }
}
