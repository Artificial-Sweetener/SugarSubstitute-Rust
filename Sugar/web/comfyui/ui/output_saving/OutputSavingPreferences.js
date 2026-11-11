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
/** Validate the shared output-saving settings contract before exposing policy to browser controls. */
import { isRecord } from '../types/common.js';
/** Narrow complete server data without inventing missing values or silently changing migrated choices. */
export function decodeOutputSavingPreferences(value) {
    if (!isRecord(value) || value.format !== 'sugarcubes.output_saving' || value.schema_version !== 1)
        throw new Error('The output saving settings format is invalid.');
    const { organization, jpeg, transfer } = value;
    if (!isRecord(organization) ||
        (organization.output_root !== null && typeof organization.output_root !== 'string') ||
        typeof organization.path_pattern !== 'string' ||
        !isRecord(jpeg) ||
        typeof jpeg.enabled !== 'boolean' ||
        (jpeg.sizing_mode !== 'quality' && jpeg.sizing_mode !== 'target_size') ||
        typeof jpeg.quality !== 'number' ||
        !Number.isInteger(jpeg.quality) ||
        jpeg.quality < 1 ||
        jpeg.quality > 100 ||
        typeof jpeg.target_size_kib !== 'number' ||
        !Number.isSafeInteger(jpeg.target_size_kib) ||
        jpeg.target_size_kib < 1 ||
        !isRecord(transfer) ||
        (transfer.preferred_format !== 'canonical_png' &&
            transfer.preferred_format !== 'companion_jpeg') ||
        (value.persistence_mode !== 'all' && value.persistence_mode !== 'final_cube') ||
        typeof value.manage_workflow !== 'boolean' ||
        typeof value.save_previews !== 'boolean')
        throw new Error('The output saving settings response is incomplete or invalid.');
    return {
        format: value.format,
        schema_version: value.schema_version,
        organization: {
            output_root: organization.output_root,
            path_pattern: organization.path_pattern,
        },
        jpeg: {
            enabled: jpeg.enabled,
            sizing_mode: jpeg.sizing_mode,
            quality: jpeg.quality,
            target_size_kib: jpeg.target_size_kib,
        },
        transfer: { preferred_format: transfer.preferred_format },
        persistence_mode: value.persistence_mode,
        manage_workflow: value.manage_workflow,
        save_previews: value.save_previews,
    };
}
/** Require the server's literal example and token list rather than reproducing naming policy in the UI. */
export function decodeOutputSavingSnapshot(value) {
    if (!isRecord(value) ||
        typeof value.example_path !== 'string' ||
        !Array.isArray(value.tokens) ||
        !value.tokens.every((token) => typeof token === 'string'))
        throw new Error('The output saving example response is invalid.');
    return {
        preferences: decodeOutputSavingPreferences(value.preferences),
        examplePath: value.example_path,
        tokens: value.tokens.filter((token) => typeof token === 'string'),
    };
}
