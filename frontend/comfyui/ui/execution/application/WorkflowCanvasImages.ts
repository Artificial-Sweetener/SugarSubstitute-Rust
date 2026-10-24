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
/** Assign stable delivery identities once, independently of result-view revisions and history replay. */
import type { WorkflowCanvasImage } from '../domain/WorkflowCanvas.js';
import type { WorkflowRun } from '../domain/WorkflowRun.js';
import type { WorkflowMediaPayload } from '../domain/WorkflowMedia.js';
import type { OutputSourceId } from '../domain/ExecutionIdentity.js';

/** Admit each finished image once per job and output source, including overlapping jobs. */
export function workflowCanvasImages(
  run: WorkflowRun,
  source: OutputSourceId,
  images: WorkflowMediaPayload['images'],
  seen: Set<string>,
): readonly WorkflowCanvasImage[] {
  const fresh: WorkflowCanvasImage[] = [];
  for (const media of images) {
    if (!media || media.mediaKind !== 'image' || media.file.type === 'input') continue;
    const id = JSON.stringify([
      run.session.sessionId,
      run.session.generation,
      run.promptId,
      source,
      media.file.type,
      media.file.subfolder,
      media.file.filename,
    ]);
    if (seen.has(id)) continue;
    seen.add(id);
    fresh.push(Object.freeze({ id, media }));
  }
  return Object.freeze(fresh);
}
