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
/** Translate native presentation progress into the shared overlay's status vocabulary. */
/** The load-error text for a selected result the host cannot present. */
export const NATIVE_CANVAS_RESULT_FAILURE = 'The image could not be loaded.';
/** The error text for a presentation the host or page failed unexpectedly. */
export const NATIVE_CANVAS_PRESENTATION_FAILURE = 'Native canvas presentation failed.';
/**
 * Report what a newly desired presentation shows until the host presents it: shown content
 * loads on an available host, waits silently for first surface discovery, and is otherwise
 * unavailable; content with nothing to show is empty.
 */
export function nativeCanvasPendingStatus(desired, host, surfacePending) {
    if (!host && !surfacePending())
        return { state: 'unavailable', message: 'The native canvas is unavailable.' };
    if (!desired.shown)
        return { state: 'empty', message: '' };
    if (host)
        return { state: 'loading', message: 'Loading the selected image.' };
    return surfacePending()
        ? { state: 'loading', message: '' }
        : { state: 'unavailable', message: 'The native canvas is unavailable.' };
}
/**
 * Report a converged presentation: shown content, a displayed preview or the final retained
 * behind an observed preview lane is ready; a preview still preparing loads; otherwise empty.
 */
export function nativeCanvasSettledStatus(desired, preview, selectionRevision) {
    const ready = desired.shown ||
        preview?.displayed ||
        (desired.previewable && preview?.observing && selectionRevision !== null);
    return { state: ready ? 'ready' : preview?.preparing ? 'loading' : 'empty', message: '' };
}
/** Describe a rejected host operation, using `message` instead of the host's when it names the result. */
export function nativeCanvasFailureStatus(result, message) {
    const error = result.status === 'error';
    return {
        state: error ? 'error' : 'unavailable',
        message: message ||
            result.message ||
            (error ? NATIVE_CANVAS_PRESENTATION_FAILURE : 'The native canvas is unavailable.'),
    };
}
