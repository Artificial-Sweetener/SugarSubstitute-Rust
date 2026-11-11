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
/** Decode public Comfy progress events before they reach the progress projection. */
import { isRecord } from '../../types/common.js';
import { comfyEventDetail, decodeComfyPromptId, decodeComfyExecutionPaths, } from './ComfyExecutionEventDecoder.js';
/** Preserve exact job identity and finite sampler values from the public event boundary. */
export function applyComfySamplerProgress(event, progress) {
    const detail = comfyEventDetail(event);
    const id = decodeComfyPromptId(detail?.prompt_id);
    const path = decodeComfyExecutionPaths([detail?.node])[0];
    if (id && path && typeof detail?.value === 'number' && typeof detail.max === 'number')
        progress.sampler(id, path, detail.value, detail.max);
}
/** Consume backend node states without treating its non-pending subset as the workflow total. */
export function applyComfyProgressState(event, progress) {
    const detail = comfyEventDetail(event);
    const id = decodeComfyPromptId(detail?.prompt_id);
    if (!id || !detail || !isRecord(detail.nodes) || Array.isArray(detail.nodes))
        return;
    const finished = [];
    for (const [path, value] of Object.entries(detail.nodes)) {
        if (!isRecord(value) || Array.isArray(value) || !decodeComfyExecutionPaths([path]).length)
            continue;
        if (value.state === 'finished')
            finished.push(path);
        if (value.state === 'running' &&
            typeof value.value === 'number' &&
            typeof value.max === 'number')
            progress.sampler(id, path, value.value, value.max);
    }
    progress.nodeFinished(id, finished);
}
