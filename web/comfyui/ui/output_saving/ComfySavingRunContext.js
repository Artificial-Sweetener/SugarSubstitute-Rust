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
/** Attach the construction-time name to the execution envelope and recipe copy without changing the host graph. */
import { isRecord } from '../types/common.js';
/** Preserve caller data while supporting Comfy's native recipe-only metadata and SugarCubes' direct envelope. */
export function attachSavingRunContext(payload, context) {
    if (!context || !isRecord(payload))
        return payload;
    const title = context.session.title.replace(/\.json$/i, '').trim();
    if (!title)
        return payload;
    const extra = isRecord(payload.extra_data) ? payload.extra_data : {};
    const saving = isRecord(extra.sugarcubes_output_saving) ? extra.sugarcubes_output_saving : {};
    const workflow = isRecord(payload.workflow) ? payload.workflow : null;
    const recipeExtra = workflow && isRecord(workflow.extra) ? workflow.extra : {};
    const recipeSaving = isRecord(recipeExtra.sugarcubes_output_saving)
        ? recipeExtra.sugarcubes_output_saving
        : {};
    return {
        ...payload,
        ...(workflow
            ? {
                workflow: {
                    ...workflow,
                    extra: {
                        ...recipeExtra,
                        sugarcubes_output_saving: { ...recipeSaving, workflow_name: title },
                    },
                },
            }
            : {}),
        extra_data: { ...extra, sugarcubes_output_saving: { ...saving, workflow_name: title } },
    };
}
