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
/** Derive the native presentation requested by the view owners, independent of host progress. */
/**
 * Qualify the selected output, optional B image or overview grid against drawable geometry.
 *
 * A comparison is presented only with an eligible final image A and an image B, so a host that
 * receives the pair can always place the divider. Overview replaces the selected image; with no
 * paintable cell or drawable geometry it presents nothing rather than a retained final.
 * Input presents its workflow-owned document under the same geometry without selecting an output.
 * Preparation permits hidden staged geometry.
 */
export function nativeCanvasDesired(geometry, item, comparison, overview, accepts, preparing, inputShown) {
    const sized = !!geometry && geometry.viewport.width > 0 && geometry.viewport.height > 0;
    const presentable = sized && geometry.visible && geometry.clip.width > 0 && geometry.clip.height > 0;
    const drawable = presentable || (sized && preparing);
    const eligible = !inputShown &&
        drawable &&
        !overview &&
        item?.mediaKind === 'image' &&
        item.file.type !== 'input'
        ? item
        : null;
    const grid = inputShown ? null : overview && (drawable ? overview : []);
    const pair = eligible && comparison?.item.mediaKind === 'image' && comparison.item.file.type !== 'input'
        ? comparison
        : null;
    const content = JSON.stringify([
        inputShown ? 'input' : 'output',
        !inputShown && item && [item.id, item.file, item.mediaKind],
        accepts,
        ...(pair ? [[pair.item.id, pair.item.file]] : []),
        ...(grid ? [['overview', grid.length > 0]] : []),
    ]);
    return {
        input: inputShown,
        geometry,
        item: eligible,
        comparison: pair,
        overview: grid,
        content,
        key: JSON.stringify([
            // Focus, gesture occlusions and the modal flag follow a presentation without replacing it.
            geometry && { ...geometry, focused: false, occlusions: undefined, modal: undefined },
            content,
        ]),
        presentable,
        empty: !inputShown &&
            (grid ? grid.length === 0 : item?.mediaKind !== 'image' || item.file.type === 'input'),
        previewable: !inputShown && !grid && item === null,
        shown: inputShown ? drawable : eligible !== null || (grid?.length ?? 0) > 0,
    };
}
/**
 * Report whether the host shows a navigable final camera: the selected final, or the final it
 * retains while a live-preview lane awaits pixels. Accepted preview pixels lock navigation to Fit,
 * and an overview grid has no camera.
 */
export function nativeCanvasNavigable(desired, preview, selectionRevision) {
    return (!preview?.displayed &&
        (!!desired?.item ||
            (!!desired?.previewable && !!preview?.observing && selectionRevision !== null)));
}
/** Report whether a host observation names exactly this attachment. */
export function nativeCanvasAddressed(attachment, observed) {
    return ['sessionId', 'viewId', 'windowId', 'attachmentId', 'generation'].every((key) => observed[key] === attachment[key]);
}
/** Identify the painted overview layout independently of the identity that entered overview. */
export function nativeCanvasOverviewKey(overview) {
    return overview && JSON.stringify(overview);
}
/** Identify divider placement independently of the pair that the select operation fixed. */
export function nativeCanvasWipeKey(comparison) {
    return comparison && JSON.stringify([comparison.orientation, comparison.split]);
}
/**
 * Select A and the optional B together so the host applies both under one revision, or select
 * the overview grid in their place.
 *
 * An overview is the displayed selection while shown: its key never equals an image key, so a
 * later image selection is always sent, and each newer layout is sent behind the one in flight.
 * An image selection carries the inspection its first frame restores; that camera is not part
 * of the key, so a changed saved camera never re-sends the shown selection.
 */
export function nativeCanvasSelection(desired, inspection) {
    const { item, comparison, overview } = desired;
    if (overview?.length)
        return {
            key: JSON.stringify(['overview', overview]),
            operation: { type: 'surface.overview', cells: overview },
        };
    if (!item)
        return null;
    const restored = inspection?.(item, comparison !== null);
    return {
        key: JSON.stringify(comparison
            ? [item.id, item.file, comparison.item.id, comparison.item.file]
            : [item.id, item.file]),
        operation: {
            type: 'surface.select',
            itemId: item.id,
            file: item.file,
            ...(comparison
                ? { comparison: { itemId: comparison.item.id, file: comparison.item.file } }
                : {}),
            ...(restored ? { inspection: restored } : {}),
        },
    };
}
/** Express the newest divider for a presented pair as the negotiated viewport command. */
export function nativeCanvasWipe(desired) {
    const { item, comparison } = desired;
    const key = nativeCanvasWipeKey(comparison);
    return item && comparison && key
        ? {
            key,
            command: { type: 'wipe', orientation: comparison.orientation, split: comparison.split },
        }
        : null;
}
