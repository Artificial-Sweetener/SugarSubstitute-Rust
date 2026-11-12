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
/** Prepare immutable restored result descriptors without constructing a live job or editor delivery. */
import type { WorkflowOutputSet } from '../domain/WorkflowOutput.js';
import type { WorkflowMediaPayload, WorkflowMediaReference } from '../domain/WorkflowMedia.js';
import { freezeWorkflowMedia } from '../domain/WorkflowMedia.js';
import type { WorkflowResultSession } from '../domain/WorkflowResultSession.js';
import type { WorkflowSessionContext } from '../domain/WorkflowSession.js';

export interface RetainedWorkflowOutput {
  readonly output: WorkflowOutputSet;
  readonly payload: WorkflowMediaPayload;
  readonly delivery: string;
  readonly completedPreviewRunOrder: number;
}

/** Describe the semantic publication accepted by the result owner for one fresh runtime generation. */
export type RestoredWorkflowResults = Pick<
  WorkflowResultSession,
  'outputs' | 'scenes' | 'runOrder' | 'presentedRunOrder'
>;

/** Copy imported references and preserve sparse batch positions for the next genuine live result update. */
export function restoredWorkflowOutput(
  context: WorkflowSessionContext,
  value: WorkflowOutputSet,
): RetainedWorkflowOutput {
  if (!Number.isSafeInteger(value.batchCount) || value.batchCount < 0 || value.batchCount > 8192)
    throw new TypeError('Restored batch exceeds the admitted desktop manifest.');
  if (
    value.items.some(
      (item) =>
        item.provenance.sessionId !== context.sessionId ||
        item.provenance.generation !== context.generation ||
        item.provenance.kind !== 'desktop-session',
    )
  )
    throw new TypeError('Restored output does not belong to this fresh workflow generation.');
  const items = Object.freeze(
    value.items.map((item) =>
      Object.freeze({
        ...item,
        file: Object.freeze({ ...item.file }),
        dimensions: item.dimensions ? Object.freeze({ ...item.dimensions }) : null,
        provenance: Object.freeze({ ...item.provenance }),
        ...(item.artifact
          ? {
              artifact: Object.freeze({
                output: Object.freeze({
                  ...item.artifact.output,
                  file: Object.freeze({ ...item.artifact.output.file }),
                  scene: item.artifact.output.scene
                    ? Object.freeze({ ...item.artifact.output.scene })
                    : null,
                }),
                binding: Object.freeze({ ...item.artifact.binding }),
              }),
            }
          : {}),
      }),
    ),
  );
  const output: WorkflowOutputSet = Object.freeze({
    ...value,
    source: Object.freeze({
      ...value.source,
      breadcrumb: Object.freeze([...value.source.breadcrumb]),
    }),
    provenance: Object.freeze({ ...value.provenance }),
    items,
    text: Object.freeze([...value.text]),
  });
  const media: (WorkflowMediaReference | null)[] = Array.from(
    { length: output.batchCount },
    () => null,
  );
  for (const item of items) {
    if (
      !Number.isSafeInteger(item.batchIndex) ||
      item.batchIndex < 0 ||
      item.batchIndex >= output.batchCount ||
      media[item.batchIndex] !== null
    )
      throw new TypeError('Restored output batch position is invalid.');
    media[item.batchIndex] = item;
  }
  return {
    output,
    payload: freezeWorkflowMedia({ images: media, videos: [], models: [], text: output.text }),
    delivery: 'desktop-session',
    completedPreviewRunOrder: items.reduce(
      (latest, item) =>
        item.batchIndex === 0 && item.file.type !== 'input'
          ? Math.max(latest, item.provenance.runOrder)
          : latest,
      -1,
    ),
  };
}
