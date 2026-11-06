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
/** Describe shared controls from native presentation facts without borrowing native resources. */
import { canvasComparisonAvailable } from '../application/CanvasComparisonSelection.js';
import { canvasSecondaryPhysicalScale, canvasSecondaryResource, canvasViewSnapshot, } from '../application/CanvasViewSnapshot.js';
/**
 * Locate the single image the host paints, from its camera: the image point at the viewport's
 * centre and its physical pixels per image pixel. Null for grids, pairs and unmeasured surfaces.
 */
function nativeBackdrop(state, item, image) {
    const { viewport, geometry } = state;
    if (!image || !item || item.mediaKind !== 'image' || !viewport || !geometry?.visible)
        return null;
    const scale = viewport.physicalScale / geometry.physicalPixelsPerCssPixel;
    const width = viewport.dimensions.width * scale;
    const height = viewport.dimensions.height * scale;
    if (!(width > 0 && height > 0))
        return null;
    const center = viewport.center ?? { x: 0.5, y: 0.5 };
    return Object.freeze({
        url: state.backdropUrl(item),
        rect: Object.freeze({
            left: geometry.origin.x + geometry.viewport.width / 2 - center.x * width,
            top: geometry.origin.y + geometry.viewport.height / 2 - center.y * height,
            width,
            height,
        }),
    });
}
/**
 * Publish only states the native surface can present.
 *
 * Without an overview host the surface presents image detail only, so overview and batch grids
 * stay hidden. Like the browser grid, a shown overview reports no image readiness, only native
 * failures. Without a negotiated comparison host, the comparison and image B are withheld even
 * if the shared view retains one from another presenter. A shown pair shares the presentation's
 * readiness.
 */
export function nativeCanvasSnapshot(state) {
    const { plan, viewport } = state;
    const item = plan.primary?.item;
    const presented = {
        ...state.presentation,
        canRetry: false,
        canAdmitLargeImage: false,
    };
    let resource = presented;
    if (item && item.mediaKind !== 'image') {
        resource = {
            ...resource,
            state: 'unavailable',
            message: 'Open this media in the host viewer.',
        };
    }
    else if (!item && plan.preview && !state.previewAvailable) {
        resource = {
            ...resource,
            state: 'unavailable',
            message: 'Live previews are not available on this surface.',
        };
    }
    else if (plan.primary &&
        !item &&
        resource.state !== 'ready' &&
        !(plan.preview && state.previewAvailable)) {
        resource = {
            ...resource,
            state: 'unavailable',
            message: 'The selected workflow output is unavailable.',
        };
    }
    const overview = state.overview && plan.view.mode === 'overview';
    if (overview && presented.state !== 'error' && presented.state !== 'unavailable')
        resource = { ...presented, state: 'empty', message: '' };
    else if (overview)
        resource = presented;
    const comparison = state.comparison ? plan.view.comparison : null;
    const secondaryDimensions = (comparison && viewport?.secondaryDimensions) || null;
    const snapshot = canvasViewSnapshot(state.address, plan, state.catalog, state.sources, state.connected, state.mediaActions, {
        comparisonAvailable: state.comparison && canvasComparisonAvailable(state.catalog),
        preview: state.preview,
        resource,
        secondaryResource: comparison
            ? canvasSecondaryResource(plan, plan.secondary?.item ? presented : undefined)
            : null,
        zoom: viewport
            ? {
                mode: viewport.mode,
                physicalScale: viewport.physicalScale,
                dimensions: viewport.dimensions,
                secondaryPhysicalScale: secondaryDimensions
                    ? canvasSecondaryPhysicalScale(viewport.physicalScale, viewport.dimensions, secondaryDimensions)
                    : null,
                secondaryDimensions,
            }
            : null,
    });
    return Object.freeze({
        ...snapshot,
        backdrop: nativeBackdrop(state, item, !overview && !comparison),
        secondary: comparison ? snapshot.secondary : null,
        grid: overview ? snapshot.grid : Object.freeze([]),
        view: Object.freeze({
            ...snapshot.view,
            mode: overview ? 'overview' : 'image',
            comparison,
        }),
    });
}
