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
 * Qualify the selected output and optional B image against drawable geometry.
 *
 * A comparison is presented only with an eligible final image A and an image B, so a host that
 * receives the pair can always place the divider. Preparation permits hidden staged geometry.
 */
export function nativeCanvasDesired(geometry, item, comparison, accepts, preparing) {
    const eligible = geometry &&
        geometry.viewport.width > 0 &&
        geometry.viewport.height > 0 &&
        (preparing || (geometry.visible && geometry.clip.width > 0 && geometry.clip.height > 0)) &&
        item?.mediaKind === 'image'
        ? item
        : null;
    const pair = eligible && comparison?.item.mediaKind === 'image' ? comparison : null;
    return {
        geometry,
        item: eligible,
        comparison: pair,
        key: JSON.stringify([
            geometry && { ...geometry, focused: false },
            item && [item.id, item.file, item.mediaKind],
            accepts,
            ...(pair ? [[pair.item.id, pair.item.file]] : []),
        ]),
        empty: item?.mediaKind !== 'image',
        previewable: item === null,
    };
}
/** Identify divider placement independently of the pair that the select operation fixed. */
export function nativeCanvasWipeKey(comparison) {
    return comparison && JSON.stringify([comparison.orientation, comparison.split]);
}
/** Select A and the optional B together so the host applies both under one revision. */
export function nativeCanvasSelection(desired) {
    const { item, comparison } = desired;
    if (!item)
        return null;
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
