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
/** Define portable, validated mask preparation intent independently of controls and native editors. */
import { isRecord } from '../../types/common.js';
/** Preserve the captured image and mask until the user requests preparation. */
export const DEFAULT_MASK_EDITS = Object.freeze({
    mask_dilation: 0,
    crop_factor: 0,
    post_dilation: 0,
    feather: 0,
});
/** Validate persisted or interactive settings before accepting processing work. */
export function readMaskEditSettings(value) {
    if (value === undefined)
        return DEFAULT_MASK_EDITS;
    if (!isRecord(value))
        throw new Error('Invalid mask edits.');
    const { mask_dilation, crop_factor, post_dilation, feather } = value;
    if (typeof mask_dilation !== 'number' ||
        !Number.isInteger(mask_dilation) ||
        Math.abs(mask_dilation) > 512 ||
        typeof post_dilation !== 'number' ||
        !Number.isInteger(post_dilation) ||
        Math.abs(post_dilation) > 512 ||
        typeof feather !== 'number' ||
        !Number.isInteger(feather) ||
        feather < 0 ||
        feather > 512 ||
        typeof crop_factor !== 'number' ||
        !Number.isFinite(crop_factor) ||
        crop_factor > 100 ||
        (crop_factor !== 0 && crop_factor < 1))
        throw new Error('Invalid mask edits.');
    return Object.freeze({ mask_dilation, crop_factor, post_dilation, feather });
}
/** Avoid unnecessary source decoding and uploads for unchanged loader inputs. */
export function hasMaskEdits(settings) {
    return (settings.mask_dilation !== 0 ||
        settings.crop_factor !== 0 ||
        settings.post_dilation !== 0 ||
        settings.feather !== 0);
}
