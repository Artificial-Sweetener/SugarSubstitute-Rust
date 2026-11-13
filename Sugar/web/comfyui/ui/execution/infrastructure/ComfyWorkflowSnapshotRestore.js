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
/** Restore media only from the named host workflow's own topology and output snapshot. */
import { isRecord } from '../../types/common.js';
import { decodeComfyWorkflowSources } from './ComfyWorkflowSourceDecoder.js';
import { decodeComfyMedia } from './ComfyMediaDecoder.js';
/** Require source membership in this workflow rather than matching a global history or node-output key. */
export function restoreComfyWorkflowSnapshot(context, adapter, results) {
    const outputs = adapter.readStoredOutputs(context.sessionId);
    if (!isRecord(outputs) || Array.isArray(outputs))
        return;
    const sources = decodeComfyWorkflowSources(adapter.readStoredWorkflow(context.sessionId));
    for (const source of sources) {
        const media = decodeComfyMedia(outputs[source.executionPath]);
        if (media)
            results.restore(context, source, media.payload, `host:${context.sessionId}:${context.generation}`);
    }
}
