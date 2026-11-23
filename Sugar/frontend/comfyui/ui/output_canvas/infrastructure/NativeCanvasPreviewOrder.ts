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
/**
 * Number the native host's preview ordering domains for this page.
 *
 * The host refuses a preview older than the newest it admitted with the same reported
 * generation, and keeps that record for the page's whole navigation. Run order and generation
 * are counted per workflow session, so each session's generation is reported as a number of its
 * own: frames of one tab stay ordered, and another tab's runs are never compared with them.
 */

import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';

const domains = new Map<string, number>();

/** The generation to report for `preview`: one number per workflow session and generation. */
export function previewOrderGeneration(
  preview: Pick<WorkflowPreview, 'sessionId' | 'generation'>,
): number {
  const key = `${preview.sessionId}\u0000${String(preview.generation)}`;
  let generation = domains.get(key);
  if (generation === undefined) {
    generation = domains.size;
    domains.set(key, generation);
  }
  return generation;
}
