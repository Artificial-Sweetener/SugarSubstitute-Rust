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
/** Prove preview admission drops work before allocation and keeps no idle loops. */

import {
  canvasImageFixture,
  canvasImageInput,
  settleCanvasImages,
} from '../support/CanvasImageFixture.js';

describe('canvas preview backpressure', () => {
  test('offers stream bytes only to an already visible slot without resetting its final partner', async () => {
    const f = canvasImageFixture();
    expect(f.resources.offer('missing', canvasImageInput('ignored', true))).toBe(false);
    f.resources.update([
      { slot: 'a', input: canvasImageInput('final-a') },
      { slot: 'b', input: canvasImageInput('final-b') },
    ]);
    f.port.ready(0);
    f.port.ready(1);
    await settleCanvasImages();
    expect(
      f.resources.offer('a', { ...canvasImageInput('preview-a', true), fallbackKey: 'final-a' }),
    ).toBe(true);
    f.port.ready(2);
    await settleCanvasImages();
    expect(f.resources.read().map((slot) => slot.frame?.key)).toEqual(['preview-a', 'final-b']);
    f.resources.active(false);
    expect(f.resources.offer('a', canvasImageInput('hidden', true))).toBe(false);
    expect(f.port.urls).toBe(0);
    f.resources.dispose();
  });
  test('retains one latest queued frame while completing an already useful decode', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('first', true) }]);
    for (let frame = 0; frame < 100; frame++)
      f.resources.update([{ slot: 'a', input: canvasImageInput(String(frame), true) }]);
    expect(f.port.requests).toHaveLength(1);
    expect(f.resources.diagnostics()).toMatchObject({
      pendingDecodes: 1,
      queuedInputs: 1,
      previewDrops: 99,
    });
    f.port.ready(0);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('first');
    expect(f.port.requests).toHaveLength(1);
    f.clock.advance(67);
    expect(f.port.requests[1]?.key).toBe('99');
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.port.urls).toBe(1);
    expect(f.resources.read()[0]?.frame?.key).toBe('99');
    expect(f.clock.pending).toBe(0);
    f.resources.dispose();
    expect(f.port.urls).toBe(0);
  });

  test('bypasses preview cadence for final delivery and revokes transient frames immediately', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('preview', true) }]);
    f.port.ready(0);
    await settleCanvasImages();
    f.resources.update([{ slot: 'a', input: canvasImageInput('next-preview', true) }]);
    expect(f.clock.pending).toBe(1);
    f.resources.update([{ slot: 'a', input: canvasImageInput('final') }]);
    expect(f.port.urls).toBe(0);
    expect(f.port.requests[1]?.key).toBe('final');
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.preview).toBe(false);
    expect(f.clock.pending).toBe(0);
    f.resources.dispose();
  });

  test('blocks repeated unknown oversized stream frames after measuring one provisional frame', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('huge', true, null) }]);
    f.port.ready(0, { width: 16384, height: 16384 });
    await settleCanvasImages();
    expect(f.port.urls).toBe(0);
    expect(f.resources.read()[0]?.status.canAdmitLargeImage).toBe(true);
    for (let frame = 0; frame < 100; frame++)
      f.resources.update([{ slot: 'a', input: canvasImageInput(String(frame), true, null) }]);
    expect(f.port.requests).toHaveLength(1);
    expect(f.resources.diagnostics().queuedInputs).toBe(1);
    f.clock.advance(67);
    expect(f.clock.pending).toBe(0);
    f.budget.allowance(16384 * 16384 * 4);
    f.resources.admissionChanged();
    expect(f.port.requests[1]?.key).toBe('99');
    f.port.ready(1, { width: 16384, height: 16384 });
    await settleCanvasImages();
    expect(f.resources.diagnostics().displayed).toBe(1);
    f.resources.update([{ slot: 'a', input: canvasImageInput('100', true, null) }]);
    f.clock.advance(67);
    expect(f.port.urls).toBe(1);
    expect(f.resources.diagnostics().displayed).toBe(0);
    f.resources.dispose();
  });

  test('rejects obsolete source/run completion without dropping a new lane', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('old', true) }]);
    f.resources.update([
      { slot: 'a', input: { ...canvasImageInput('new', true), lane: 'preview:new-run' } },
    ]);
    f.port.ready(0);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('new');
    expect(f.port.urls).toBe(1);
    f.resources.dispose();
  });

  test('invalid intrinsic dimensions fail visibly without poisoning an exact retry', async () => {
    const f = canvasImageFixture();
    const inputs = [{ slot: 'a', input: canvasImageInput('a', false, null) }];
    f.resources.update(inputs);
    f.port.ready(0, { width: 0, height: 0 });
    await settleCanvasImages();
    expect(f.resources.read()[0]?.status.canRetry).toBe(true);
    expect(f.resources.dimensions('a')).toBeNull();
    expect(f.budget.unknownLoads).toBe(0);
    f.resources.update(inputs, true);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('a');
    f.resources.dispose();
  });
});
