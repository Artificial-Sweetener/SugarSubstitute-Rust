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
 * Publish only states the native surface can present.
 *
 * The surface presents image detail only, so overview and batch grids stay hidden. Without a
 * negotiated comparison host, the comparison and image B are withheld even if the shared view
 * retains one from another presenter. A shown pair shares the presentation's readiness.
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
        secondary: comparison ? snapshot.secondary : null,
        grid: Object.freeze([]),
        view: Object.freeze({ ...snapshot.view, mode: 'image', comparison }),
    });
}
