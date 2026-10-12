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
/** Project current source membership and authoritative execution results into stable navigation. */
import { canvasSessionId } from '../domain/CanvasIdentity.js';
/** Include only declared workflow sources with attributable media or a current live preview. */
export function canvasOutputCatalog(result, inventory) {
    const outputs = new Map(result.outputs.map((output) => [output.source.id, output]));
    const entries = [];
    for (const source of [...inventory].sort((a, b) => a.order - b.order)) {
        const output = outputs.get(source.id);
        const preview = result.preview?.sourceId === source.id ? result.preview : null;
        if (!preview && !output?.batchCount)
            continue;
        entries.push(Object.freeze({
            source,
            items: output?.items ?? Object.freeze([]),
            preview,
            summary: Object.freeze({
                id: source.id,
                label: source.label,
                breadcrumb: source.breadcrumb,
                batchCount: output?.batchCount ?? 0,
                preview: Boolean(preview),
                previousRun: Boolean(output?.items.some((item) => item.provenance.runOrder < result.runOrder)),
            }),
        }));
    }
    const labels = new Map();
    for (const entry of entries)
        labels.set(entry.summary.label, (labels.get(entry.summary.label) ?? 0) + 1);
    const occurrences = new Map();
    const captions = new Map();
    for (const entry of entries) {
        const caption = [...entry.source.breadcrumb, entry.source.label].join(' / ');
        captions.set(caption, (captions.get(caption) ?? 0) + 1);
    }
    const qualified = entries.map((entry) => {
        if ((labels.get(entry.summary.label) ?? 0) <= 1)
            return entry;
        const label = [...entry.source.breadcrumb, entry.source.label].join(' / ');
        const occurrence = (occurrences.get(label) ?? 0) + 1;
        occurrences.set(label, occurrence);
        const repeated = (captions.get(label) ?? 0) > 1;
        return Object.freeze({
            ...entry,
            summary: Object.freeze({
                ...entry.summary,
                label: repeated ? `${label} (${occurrence})` : label,
            }),
        });
    });
    return Object.freeze({
        sessionId: canvasSessionId(result.context.sessionId),
        generation: result.context.generation,
        entries: Object.freeze(qualified),
        result,
    });
}
