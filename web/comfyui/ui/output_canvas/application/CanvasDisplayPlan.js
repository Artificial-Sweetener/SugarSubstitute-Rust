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
/** Describe visible logical image interests without retaining platform handles or preview bytes. */
import { canvasBatchGridCatalog } from './CanvasContentSelection.js';
/** Qualify lightweight preview admission to one source/run rather than each transport frame. */
export function canvasPreviewLane(preview) {
    return JSON.stringify([
        'preview',
        preview.sessionId,
        preview.generation,
        preview.promptId,
        preview.sourceId,
        preview.runOrder,
    ]);
}
/** Keep camera, wipe, labels, and preview sequence changes out of complete resource-plan replacement. */
export function canvasDisplayPlan(view, catalog, items, preview) {
    if (view.sessionId !== catalog.sessionId)
        throw new TypeError('Canvas display catalog belongs to another view.');
    const slot = (selection, item) => selection
        ? Object.freeze({
            selection,
            item,
            sourcePresent: catalog.entries.some((entry) => entry.source.id === selection.sourceId),
        })
        : null;
    const primary = slot(view.selection, items.primary);
    const secondary = slot(view.comparison?.secondary ?? null, items.secondary);
    const availableSources = Object.freeze(catalog.entries.map((entry) => entry.source.id));
    const grid = view.mode !== 'overview'
        ? Object.freeze([])
        : catalog.sceneOverview
            ? catalog.entries
            : canvasBatchGridCatalog(catalog, view.selection, view.follow === 'auto').entries;
    const signature = JSON.stringify([
        catalog.generation,
        view.mode,
        grid.map((entry) => [
            entry.scene ?? null,
            entry.source.id,
            entry.items.map((item) => item.id),
            entry.preview ? canvasPreviewLane(entry.preview) : null,
        ]),
        availableSources,
        primary?.selection,
        primary?.sourcePresent,
        primary?.item?.id,
        secondary?.selection,
        secondary?.sourcePresent,
        secondary?.item?.id,
        preview ? canvasPreviewLane(preview) : null,
    ]);
    return Object.freeze({
        view,
        generation: catalog.generation,
        signature,
        availableSources,
        grid,
        primary,
        secondary,
        preview,
    });
}
