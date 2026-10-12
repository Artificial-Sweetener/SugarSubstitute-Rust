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

import type { WorkflowOutputSource } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowResultSession } from '../../execution/domain/WorkflowResultSession.js';
import type { CanvasCatalogEntry, CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import { canvasSessionId } from '../domain/CanvasIdentity.js';

/** Retain displayed workflow media until the first visual hands over to the current Run. */
export function canvasOutputCatalog(
  result: WorkflowResultSession,
  inventory: readonly WorkflowOutputSource[],
): CanvasCatalogSnapshot {
  const presentedRun = result.outputs.reduce(
    (latest, output) =>
      output.items.reduce(
        (run, item) => (item.file.type === 'input' ? run : Math.max(run, item.provenance.runOrder)),
        latest,
      ),
    Math.max(result.presentedRunOrder, result.preview?.runOrder ?? 0),
  );
  const outputs = new Map(result.outputs.map((output) => [output.source.id, output]));
  const entries: CanvasCatalogEntry[] = [];
  for (const source of [...inventory].sort((a, b) => a.order - b.order)) {
    const output = outputs.get(source.id);
    const input = Boolean(
      output?.items.length && output.items.every((item) => item.file.type === 'input'),
    );
    const items =
      output?.items.filter(
        (item) => (input && presentedRun === 0) || item.provenance.runOrder === presentedRun,
      ) ?? [];
    const preview = result.preview?.sourceId === source.id ? result.preview : null;
    if (!preview && !items.length) continue;
    entries.push(
      Object.freeze({
        source,
        items: Object.freeze(items),
        preview,
        summary: Object.freeze({
          id: source.id,
          label: source.label,
          breadcrumb: source.breadcrumb,
          batchCount: items.length ? (output?.batchCount ?? 0) : 0,
          preview: Boolean(preview),
          previousRun: Boolean(items.some((item) => item.provenance.runOrder < result.runOrder)),
        }),
      }),
    );
  }
  return Object.freeze({
    sessionId: canvasSessionId(result.context.sessionId),
    generation: result.context.generation,
    entries: Object.freeze(qualifiedCaptions(entries)),
    result,
  });
}

/** Disambiguate collisions within the output catalog without prefixing every authored source caption. */
function qualifiedCaptions(entries: readonly CanvasCatalogEntry[]): readonly CanvasCatalogEntry[] {
  const labels = new Map<string, number>();
  for (const entry of entries)
    labels.set(entry.summary.label, (labels.get(entry.summary.label) ?? 0) + 1);
  const occurrences = new Map<string, number>();
  const captions = new Map<string, number>();
  for (const entry of entries) {
    const caption = [...entry.source.breadcrumb, entry.source.label].join(' / ');
    captions.set(caption, (captions.get(caption) ?? 0) + 1);
  }
  return entries.map((entry) => {
    if ((labels.get(entry.summary.label) ?? 0) <= 1) return entry;
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
}
