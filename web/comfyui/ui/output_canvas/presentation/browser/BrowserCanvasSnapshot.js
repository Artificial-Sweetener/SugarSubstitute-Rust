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
/** Describe actual displayed image fidelity and loading state for reusable overlay controls. */
/** Project only lightweight status, keeping DOM handles out of the renderer-independent port. */
export function browserCanvasSnapshot(address, plan, catalog, images, camera, connected, previewStarted) {
    const primary = images.find((slot) => slot.slot === 'primary');
    const secondary = images.find((slot) => slot.slot === 'secondary');
    const frame = primary?.frame;
    const preview = Boolean(frame?.preview && plan.preview);
    let resource = primary?.status ?? status('empty', 'Choose a workflow output.');
    if (plan.primary && !plan.primary.sourcePresent)
        resource = status('unavailable', 'The selected workflow output is unavailable.');
    else if (plan.primary?.item && plan.primary.item.mediaKind !== 'image')
        resource = status('unavailable', 'Open this media in the host viewer.');
    else if (plan.preview && !previewStarted)
        resource = status('loading', frame
            ? 'Waiting for a fresh source preview; showing the final image.'
            : 'Waiting for a fresh source preview.');
    else if (plan.preview && resource.state === 'error')
        resource = {
            ...resource,
            canRetry: false,
            message: 'The source preview failed. Waiting for a fresh frame.',
        };
    else if (plan.primary && !plan.primary.item && !plan.preview)
        resource = status('unavailable', 'The requested batch image is unavailable.');
    else if (frame &&
        !frame.preview &&
        frame.key !== plan.primary?.item?.id &&
        resource.state === 'loading')
        resource = { ...resource, message: 'Loading the selected image; showing the previous image.' };
    const physicalScale = camera ? camera.camera.scale * camera.geometry.density : 0;
    const b = secondary?.frame;
    return Object.freeze({
        schema: 1,
        address,
        revision: 0,
        view: plan.view,
        sources: Object.freeze(catalog.entries.map((entry) => entry.summary)),
        selected: plan.primary?.item ?? null,
        secondary: plan.secondary?.item ?? null,
        preview,
        resource: Object.freeze(resource),
        runStatus: catalog.result.runStatus,
        connected,
        zoom: Object.freeze({
            mode: camera?.camera.mode ?? 'fit',
            physicalScale,
            dimensions: camera?.geometry.image ?? null,
            secondaryPhysicalScale: b && camera
                ? Object.freeze({
                    x: (physicalScale * camera.geometry.image.width) / b.dimensions.width,
                    y: (physicalScale * camera.geometry.image.height) / b.dimensions.height,
                })
                : null,
            secondaryDimensions: b?.dimensions ?? null,
        }),
    });
}
/** Describe unallocated/unsupported media without creating image resources or retry promises. */
function status(state, message) {
    return { state, message, canRetry: false, canAdmitLargeImage: false };
}
