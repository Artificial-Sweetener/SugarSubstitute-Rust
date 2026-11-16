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
/** Bring back the mask pixels a generated image's workflow was queued with. */
import { WORKFLOW_WIDGET_VALUES_KEY } from '../../graph/WorkflowWidgetSnapshotCapture.js';
import { isRecord } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';
import type { InputBinding, InputEntry } from '../domain/InputBinding.js';

/**
 * Read the files a queued loader field read, as the queued workflow recorded them on its node.
 *
 * A queue replaces each canvas mask with a captured file and records those files beside the
 * loader's unchanged canvas binding. Only a workflow loaded from a generated image, or from
 * queue history, carries the record: Comfy keeps it on the live node and never serializes it.
 */
export function generatedSources(node: ComfyNode, field: string): readonly string[] | null {
  const values = node[WORKFLOW_WIDGET_VALUES_KEY];
  const value: unknown = isRecord(values) ? values[field] : undefined;
  const files: readonly unknown[] | null =
    typeof value === 'string' ? [value] : Array.isArray(value) ? value : null;
  return files?.length && files.every((file): file is string => typeof file === 'string' && !!file)
    ? files
    : null;
}

/** Forget the record once its files have been offered, so later edits are never replaced. */
export function forgetGeneratedSources(node: ComfyNode, field: string): void {
  const values = node[WORKFLOW_WIDGET_VALUES_KEY];
  if (isRecord(values))
    node[WORKFLOW_WIDGET_VALUES_KEY] = Object.fromEntries(
      Object.entries(values).filter(([name]) => name !== field),
    );
}

/**
 * Point each canvas mask at the file it was generated from, where that file is still available.
 *
 * The canvas that provisions the workflow's surfaces imports a mask's file into its layer and
 * the loader then returns to that layer, so the masks arrive whether or not the canvas still
 * holds the documents the workflow was painted in. An ordered loader's files pair with its
 * entries in order; a scalar loader's one file belongs to its selected entry. A binding whose
 * entries no longer match the record is left alone.
 *
 * @returns The changed binding, or null when no entry takes a file.
 */
export function withGeneratedSources(
  binding: InputBinding,
  ordered: boolean,
  files: readonly string[],
  available: readonly boolean[],
  provisions: (canvasId: string) => boolean,
): InputBinding | null {
  if (ordered ? files.length !== binding.entries.length : files.length !== 1) return null;
  const selected = binding.entries.findIndex((entry) => entry.id === binding.selected);
  const first = ordered ? 0 : Math.max(selected, 0);
  let changed = false;
  const entries = binding.entries.map((entry, index): InputEntry => {
    const file = index < first ? undefined : files[index - first];
    if (
      file === undefined ||
      available[index - first] !== true ||
      entry.source.kind !== 'canvas' ||
      entry.source.target.kind !== 'layer' ||
      !provisions(entry.source.canvasId)
    )
      return entry;
    changed = true;
    return { ...entry, source: { kind: 'file', file } };
  });
  return changed ? { ...binding, entries } : null;
}
