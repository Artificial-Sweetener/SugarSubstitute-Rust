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
/** Prove continuous status is bounded, finite, and scoped to its attachment. */

import { CanvasStatusPublisher } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasStatusPublisher.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

test('coalesces continuous updates without polling idle viewers and interrupts for deliberate navigation', () => {
  const clock = new WorkflowTestClock();
  let publications = 0;
  const publisher = new CanvasStatusPublisher(clock, () => {
    publications++;
  });
  publisher.changed();
  for (let update = 0; update < 100; update++) publisher.changed();
  expect(publications).toBe(1);
  expect(clock.pending).toBe(1);
  clock.advance(99);
  expect(publications).toBe(1);
  clock.advance(1);
  expect(publications).toBe(2);
  expect(clock.pending).toBe(0);
  clock.advance(10000);
  expect(publications).toBe(2);
  publisher.changed();
  publisher.changed();
  expect(clock.pending).toBe(1);
  publisher.changed(true);
  expect(publications).toBe(4);
  expect(clock.pending).toBe(0);
  publisher.changed();
  publisher.dispose();
  expect(clock.pending).toBe(0);
  clock.advance(10000);
  publisher.changed(true);
  expect(publications).toBe(4);
});

test('disposing one attachment leaves the other attachment publication intact', () => {
  const clock = new WorkflowTestClock();
  let first = 0;
  let second = 0;
  const a = new CanvasStatusPublisher(clock, () => {
    first++;
  });
  const b = new CanvasStatusPublisher(clock, () => {
    second++;
  });
  a.changed();
  b.changed();
  a.changed();
  b.changed();
  a.dispose();
  clock.advance(100);
  expect(first).toBe(1);
  expect(second).toBe(2);
  expect(clock.pending).toBe(0);
  b.dispose();
});
