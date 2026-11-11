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

/** Own stable per-source batch positions and replay suppression for one captured run. */
export class WorkflowCanvasImages {
  readonly #seen = new Set<string>();
  readonly #positions = new Map<OutputSourceId, number>();

  /** Admit each finished image immediately, retaining its mask-batch position across incremental events. */
  read(
    run: WorkflowRun,
    source: OutputSourceId,
    images: WorkflowMediaPayload['images'],
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
      if (this.#seen.has(id)) continue;
      this.#seen.add(id);
      const batchIndex = this.#positions.get(source) ?? 0;
      this.#positions.set(source, batchIndex + 1);
      fresh.push(Object.freeze({ id, media, batchIndex }));
    }
    return Object.freeze(fresh);
  }
}
