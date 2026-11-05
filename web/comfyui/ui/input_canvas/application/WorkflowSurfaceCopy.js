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
/** Hold every user-facing string of workflow input surfaces in one place. */
export const WORKFLOW_SURFACE_COPY = Object.freeze({
    inputRoute: 'Input',
    outputRoute: 'Output',
    routeLabel: 'Canvas route',
    noInputSurfaces: 'No input canvas nodes',
    maskOnlySurface: 'Mask canvas',
    visualOpacity: 'Visual opacity',
    maskDimensionsMismatchTitle: 'Mask dimensions do not match',
    maskDimensionsMismatch: (mask, image) => `The selected mask dimensions (${mask}) do not match the loaded input image (${image}).`,
    maskDimensionsUnverifiedTitle: 'Mask dimensions could not be verified',
    maskDimensionsUnverified: 'The selected mask dimensions could not be verified against the loaded input image.',
    surfaceSyncFailed: 'The input canvas could not be updated.',
    regionalMasksMissing: (mask, required, available) => `Regional prompt is missing an associated mask: ${mask} has ${String(available)} of the ${String(required)} masks its prompt regions need.`,
    chooseMask: 'Choose mask',
    chooseMaskFiles: 'Mask files',
    browseFiles: 'Browse Files',
    chooseMaskFile: 'Mask file',
    changeCanvasResolutionMenu: 'Change canvas resolution…',
    changeCanvasResolution: 'Change canvas resolution',
    changeCanvasResolutionDescription: 'Choose how the Input canvas and its regional masks should fit the new size.',
    width: 'Width',
    height: 'Height',
    resizeScope: 'Resize',
    resizeCanvasOnly: 'Resize canvas only',
    scaleCanvasAndMasks: 'Scale canvas and masks',
    anchorPoint: 'Anchor point',
    anchors: Object.freeze({
        'top-left': 'Top left',
        top: 'Top',
        'top-right': 'Top right',
        left: 'Left',
        center: 'Center',
        right: 'Right',
        'bottom-left': 'Bottom left',
        bottom: 'Bottom',
        'bottom-right': 'Bottom right',
    }),
    scalingQuality: 'Scaling quality',
    nearestNeighbor: 'Nearest Neighbor',
    smooth: 'Smooth',
    resizeCanvas: 'Resize canvas',
    scaleCanvas: 'Scale canvas',
    cancel: 'Cancel',
    resizeInvalid: 'Choose a width and height between 64 and 32768 pixels.',
    resizeStale: 'The canvas changed while this dialog was open. Review the current size and try again.',
    resizeFailed: 'The canvas could not be resized. Nothing was changed.',
    resizeWriteFailed: 'The canvas was resized, but the workflow dimensions could not be updated.',
});
