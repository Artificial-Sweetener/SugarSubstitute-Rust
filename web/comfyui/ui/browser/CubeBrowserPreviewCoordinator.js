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
/** Own revision-aware Cube preview loading and geometry projection. */
import { isCurrentRevisionRef } from '../core/CubeDefinitionKey.js';
import { planPlacementGeometry } from '../geometry/PlacementGeometryPlanner.js';
import { resolveRendererGeometryPolicy } from '../geometry/RendererGeometryPolicy.js';
import { readImportPayload } from '../import/PlacementPayload.js';
import { hasApiError, readApiErrorMessage, readErrorMessage, } from './CubeBrowserContracts.js';
/** Coordinate cached preview requests for the active Cube revision. */
export class CubeBrowserPreviewCoordinator {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Load or redraw the preview for one selected Cube. */
    async request(cubeKey) {
        const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
        const { preview, store } = this.options;
        if (!key) {
            preview.update({ name: null, requestKey: null, payload: null, loading: false, error: null });
            return;
        }
        const selected = this.options.getCube(key);
        const cubeId = selected?.cube_id || '';
        const revisionRef = store.state.selectedRevision || 'WORKTREE';
        const requestKey = `${key}::${revisionRef}`;
        const requestState = preview.getRequestState(requestKey);
        if (requestState === 'loading')
            return;
        if (requestState === 'ready') {
            preview.render();
            return;
        }
        if (!cubeId) {
            preview.update({
                name: key,
                requestKey,
                payload: null,
                loading: false,
                error: 'Cube id missing.',
            });
            return;
        }
        const requestId = (preview.requestId || 0) + 1;
        preview.requestId = requestId;
        preview.update({ name: key, requestKey, payload: null, loading: true, error: null });
        try {
            const current = isCurrentRevisionRef(revisionRef);
            const requestBody = current
                ? JSON.stringify({ cube_id: cubeId, origin: { x: 0, y: 0 } })
                : JSON.stringify({ cube_id: cubeId, revision_ref: revisionRef, origin: { x: 0, y: 0 } });
            const loader = current
                ? this.options.api.load.bind(this.options.api)
                : this.options.api.loadRevision.bind(this.options.api);
            const { response, data } = await loader(requestBody, {
                headers: { 'Content-Type': 'application/json' },
            });
            if (requestId !== preview.requestId)
                return;
            if (!response.ok || hasApiError(data)) {
                preview.update({
                    name: key,
                    requestKey,
                    payload: null,
                    loading: false,
                    error: readApiErrorMessage(data, response.statusText || 'Preview failed'),
                });
                return;
            }
            const importPayload = readImportPayload(data);
            if (!importPayload) {
                preview.update({
                    name: key,
                    requestKey,
                    payload: null,
                    loading: false,
                    error: 'Importer payload missing.',
                });
                return;
            }
            preview.update({
                name: key,
                requestKey,
                payload: planPlacementGeometry(importPayload, resolveRendererGeometryPolicy(this.options.adapter?.getLiteGraph?.(), this.options.adapter?.getNodeRenderer?.())),
                loading: false,
                error: null,
            });
        }
        catch (error) {
            if (requestId !== preview.requestId)
                return;
            preview.update({
                name: key,
                requestKey,
                payload: null,
                loading: false,
                error: readErrorMessage(error),
            });
        }
    }
}
