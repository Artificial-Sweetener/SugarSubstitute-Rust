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
/** Prove finite visible image ownership, atomic replacement and stale-load release. */

import {
  canvasImageFixture,
  canvasImageInput,
  settleCanvasImages,
} from '../support/CanvasImageFixture.js';

describe('canvas image lifetime', () => {
  test('reports known admission cost and an unknown comparison partner without allocating it', async () => {
    const f = canvasImageFixture();
    const dimensions = { width: 8192, height: 8192 };
    f.resources.update([{ slot: 'a', input: canvasImageInput('large', false, null) }]);
    f.port.ready(0, dimensions);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.status.message).toContain('256 MiB estimated RGBA storage');
    expect(f.port.allocations).toBe(0);
    f.budget.allowance(268435456);
    f.resources.update([{ slot: 'a', input: canvasImageInput('large', false, null) }], true);
    f.port.ready(1, dimensions);
    await settleCanvasImages();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('large', false, null) },
      { slot: 'b', input: canvasImageInput('unknown', false, null) },
    ]);
    expect(f.resources.read()[1]?.status.message).toContain('next image size is unknown');
    expect(f.resources.read()[1]?.status.message).toContain('256 MiB already owned');
    expect(f.port.requests).toHaveLength(2);
    f.resources.dispose();
  });
  test('clears the outgoing source while its replacement is decoding', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('old') }]);
    f.port.ready(0);
    await settleCanvasImages();
    f.resources.update([{ slot: 'a', input: canvasImageInput('new') }]);
    expect(f.resources.read()[0]?.frame).toBeNull();
    expect(f.port.allocations).toBe(1);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.selection).toEqual({
      sourceId: 'source:new',
      batchIndex: 0,
    });
    f.resources.dispose();
  });
  test('clears another batch position from the same source before replacement decode', async () => {
    const f = canvasImageFixture();
    const first = canvasImageInput('first');
    f.resources.update([{ slot: 'a', input: first }]);
    f.port.ready(0);
    await settleCanvasImages();
    f.resources.update([
      {
        slot: 'a',
        input: { ...canvasImageInput('third'), selection: { ...first.selection, batchIndex: 2 } },
      },
    ]);
    expect(f.resources.read()[0]?.frame).toBeNull();
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.selection).toEqual({ ...first.selection, batchIndex: 2 });
    f.resources.dispose();
    expect(f.port.allocations).toBe(0);
  });
  test('refuses retaining previews into an unavailable overview allowance or a foreign lane', async () => {
    for (const restricted of [true, false]) {
      const f = canvasImageFixture();
      const input = canvasImageInput('preview', true);
      f.resources.update([{ slot: 'primary', input }]);
      f.port.ready(0, { width: 512, height: 288 });
      await settleCanvasImages();
      if (restricted) f.budget.allowance(256 * 1024 * 1024);
      f.resources.update([], false, {
        slot: 'grid',
        lane: restricted ? input.lane : 'foreign-run',
        selection: input.selection,
        role: 'thumbnail',
      });
      expect(f.resources.read()).toEqual([]);
      expect(f.port.allocations).toBe(0);
      f.resources.dispose();
    }
  });
  test('moves a selected partner into single view but decodes duplicate chosen images independently', async () => {
    const f = canvasImageFixture();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('a') },
      { slot: 'b', input: canvasImageInput('b') },
    ]);
    f.port.ready(0);
    f.port.ready(1);
    await settleCanvasImages();
    f.resources.update([{ slot: 'a', input: canvasImageInput('b') }]);
    expect(f.port.requests).toHaveLength(2);
    expect(f.resources.read()[0]?.frame?.key).toBe('b');
    expect(f.port.allocations).toBe(1);
    f.resources.update([
      { slot: 'a', input: canvasImageInput('b') },
      { slot: 'b', input: canvasImageInput('b') },
    ]);
    expect(f.port.requests).toHaveLength(3);
    f.port.ready(2);
    await settleCanvasImages();
    const handles = f.resources.read().map((slot) => slot.frame?.handle);
    expect(handles[0]).not.toBe(handles[1]);
    f.resources.dispose();
    expect(f.port.allocations).toBe(0);
  });
  test('loads a 4K pair with two admissions and atomically replaces each displayed image', async () => {
    const f = canvasImageFixture();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('a') },
      { slot: 'b', input: canvasImageInput('b') },
    ]);
    expect(f.decodes.pending).toBe(2);
    expect(f.port.allocations).toBe(2);
    f.port.ready(0);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.diagnostics()).toMatchObject({
      displayed: 2,
      pendingDecodes: 0,
      activeDeadlines: 0,
    });
    f.resources.update([
      {
        slot: 'a',
        input: { ...canvasImageInput('a2'), selection: canvasImageInput('a').selection },
      },
      {
        slot: 'b',
        input: { ...canvasImageInput('b2'), selection: canvasImageInput('b').selection },
      },
    ]);
    expect(f.resources.read().map((slot) => slot.frame?.key)).toEqual(['a', 'b']);
    expect(f.port.allocations).toBe(4);
    expect(f.budget.estimatedBytes).toBe(3840 * 2160 * 4 * 4);
    f.port.ready(2);
    await settleCanvasImages();
    expect(f.resources.read().map((slot) => slot.frame?.key)).toEqual(['a2', 'b']);
    expect(f.port.allocations).toBe(3);
    f.port.ready(3);
    await settleCanvasImages();
    f.resources.dispose();
    expect(f.port.allocations).toBe(0);
    expect(f.clock.pending).toBe(0);
    expect(f.budget.estimatedBytes).toBe(0);
  });

  test('keeps one latest pending input per slot and rejects canceled late delivery', async () => {
    const f = canvasImageFixture();
    for (let index = 0; index < 100; index++)
      f.resources.update([{ slot: 'a', input: canvasImageInput(String(index)) }]);
    expect(f.resources.diagnostics()).toMatchObject({ pendingDecodes: 1, queuedInputs: 0 });
    expect(f.port.allocations).toBe(1);
    f.port.ready(0);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame).toBeNull();
    f.port.ready(99);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('99');
    f.resources.dispose();
    expect(f.port.allocations).toBe(0);
  });

  test('serializes unknown-size loads without presenting a temporary wait as a large-image approval', async () => {
    const f = canvasImageFixture();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('a', false, null) },
      { slot: 'b', input: canvasImageInput('b', false, null) },
    ]);
    expect(f.port.requests).toHaveLength(1);
    expect(f.budget.unknownLoads).toBe(1);
    expect(f.resources.read()[1]?.status.state).toBe('loading');
    expect(f.resources.read()[1]?.status.canAdmitLargeImage).toBe(false);
    f.port.ready(0);
    await settleCanvasImages();
    expect(f.port.requests).toHaveLength(2);
    expect(f.resources.read()[1]?.status.state).toBe('loading');
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.diagnostics().displayed).toBe(2);
    f.resources.dispose();
  });

  test('releases hidden attachments immediately and retains no inactive preview interest or payload queue', async () => {
    const f = canvasImageFixture();
    f.resources.update([{ slot: 'a', input: canvasImageInput('preview', true) }]);
    f.resources.active(false);
    expect(f.port.urls).toBe(0);
    expect(f.port.allocations).toBe(0);
    expect(f.clock.pending).toBe(0);
    expect(f.resources.diagnostics()).toMatchObject({
      displayed: 0,
      pendingDecodes: 0,
      queuedInputs: 0,
      estimatedBytes: 0,
    });
    f.resources.update([{ slot: 'a', input: canvasImageInput('ignored', true) }]);
    f.port.ready(0);
    await settleCanvasImages();
    expect(f.port.requests).toHaveLength(1);
    expect(f.resources.read()).toEqual([]);
    f.resources.active(true);
    f.resources.update([{ slot: 'a', input: canvasImageInput('final') }]);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('final');
    f.resources.dispose();
  });

  test('reports failure/retry without clearing a valid comparison partner or reloading it', async () => {
    const f = canvasImageFixture();
    const inputs = [
      { slot: 'a', input: canvasImageInput('a') },
      { slot: 'b', input: canvasImageInput('b') },
    ];
    f.resources.update(inputs);
    f.port.fail(0);
    f.port.ready(1);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.status).toMatchObject({ state: 'error', canRetry: true });
    expect(f.resources.read()[1]?.frame?.key).toBe('b');
    expect(f.errors).toHaveLength(1);
    f.resources.update(inputs, true);
    expect(f.port.requests).toHaveLength(3);
    f.port.ready(2);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame?.key).toBe('a');
    f.resources.dispose();
  });

  test('has finite deadlines, recovers from platform start failure and returns counters to zero', async () => {
    const f = canvasImageFixture();
    f.port.failStart = true;
    const inputs = [{ slot: 'a', input: canvasImageInput('a') }];
    f.resources.update(inputs);
    expect(f.resources.read()[0]?.status.state).toBe('error');
    expect(f.budget.estimatedBytes).toBe(0);
    f.port.failStart = false;
    f.resources.update(inputs, true);
    f.clock.advance(10000);
    expect(f.port.allocations).toBe(0);
    expect(f.decodes.pending).toBe(0);
    expect(f.clock.pending).toBe(0);
    expect(f.resources.read()[0]?.status.canRetry).toBe(true);
    f.port.ready(0);
    await settleCanvasImages();
    expect(f.resources.read()[0]?.frame).toBeNull();
    f.resources.dispose();
  });

  test('swaps an already displayed final pair without decoding duplicate images', async () => {
    const f = canvasImageFixture();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('a') },
      { slot: 'b', input: canvasImageInput('b') },
    ]);
    f.port.ready(0);
    f.port.ready(1);
    await settleCanvasImages();
    f.resources.update([
      { slot: 'a', input: canvasImageInput('b') },
      { slot: 'b', input: canvasImageInput('a') },
    ]);
    expect(f.port.requests).toHaveLength(2);
    expect(f.resources.read().map((slot) => slot.frame?.key)).toEqual(['b', 'a']);
    f.resources.dispose();
    expect(f.port.allocations).toBe(0);
  });

  test('bounds overview interest and releases offscreen cells without warming hidden workflow images', async () => {
    const f = canvasImageFixture();
    const inputs = Array.from({ length: 8 }, (_, index) => ({
      slot: `thumb:${index}`,
      input: {
        ...canvasImageInput(String(index)),
        role: 'thumbnail' as const,
        dimensions: { width: 256, height: 256 },
      },
    }));
    f.resources.update(inputs);
    expect(f.decodes.pending).toBe(2);
    for (let index = 0; index < 8; index++) {
      f.port.ready(index, { width: 256, height: 256 });
      await settleCanvasImages();
    }
    expect(f.resources.diagnostics().displayed).toBe(8);
    expect(() =>
      f.resources.update([...inputs, { slot: 'extra', input: inputs[0]!.input }]),
    ).toThrow('bounds');
    f.resources.update([]);
    expect(f.port.allocations).toBe(0);
    expect(f.budget.estimatedBytes).toBe(0);
    f.resources.dispose();
  });

  test('repeated activate/deactivate cycles plateau at zero owned inactive resources', async () => {
    const f = canvasImageFixture();
    for (let index = 0; index < 100; index++) {
      f.resources.active(true);
      f.resources.update([{ slot: 'a', input: canvasImageInput(String(index), true) }]);
      f.port.ready(index);
      await settleCanvasImages();
      f.resources.active(false);
      expect(f.resources.diagnostics()).toMatchObject({
        displayed: 0,
        pendingDecodes: 0,
        queuedInputs: 0,
        estimatedBytes: 0,
        activeDeadlines: 0,
      });
      expect(f.port.allocations).toBe(0);
      expect(f.port.urls).toBe(0);
    }
    f.resources.dispose();
  });
});
