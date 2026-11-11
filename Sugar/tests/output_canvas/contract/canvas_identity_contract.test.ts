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
/** Keep workflow, view, and attachment identities distinct at compile and runtime. */

import {
  canvasSessionId,
  canvasViewId,
  surfaceAttachmentId,
  windowId,
  type CanvasViewId,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  outputResultId,
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { CanvasViewPort } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';

type Assert<T extends true> = T;
type IsAssignable<From, To> = [From] extends [To] ? true : false;
type Negate<T extends boolean> = T extends true ? false : true;
type DistinctSession = Assert<
  Negate<IsAssignable<ReturnType<typeof workflowSessionId>, CanvasViewId>>
>;
type DistinctWindow = Assert<Negate<IsAssignable<ReturnType<typeof windowId>, CanvasViewId>>>;
type DistinctAttachment = Assert<
  Negate<IsAssignable<ReturnType<typeof surfaceAttachmentId>, CanvasViewId>>
>;
type NoImageHandles = Assert<
  Negate<IsAssignable<HTMLImageElement, ReturnType<CanvasViewPort['read']>>>
>;

test('keeps equal node IDs from different workflow sessions or jobs distinguishable', () => {
  const a = workflowSessionId('a');
  const b = workflowSessionId('b');
  const job = promptId('same-job');
  const source = outputSourceId('7:23');
  expect(outputResultId(a, job, source, 1, 0)).not.toBe(outputResultId(b, job, source, 1, 0));
  expect(outputResultId(a, job, source, 1, 0)).not.toBe(outputResultId(a, job, source, 1, 1));
  expect(outputResultId(a, job, source, 1, 0)).not.toBe(outputResultId(a, job, source, 2, 0));
  expect(canvasSessionId(a)).not.toBe(canvasSessionId(b));
  expect(canvasViewId('a')).not.toBe(windowId('a'));
  const typing: [DistinctSession, DistinctWindow, DistinctAttachment, NoImageHandles] = [
    true,
    true,
    true,
    true,
  ];
  expect(typing).toEqual([true, true, true, true]);
});
