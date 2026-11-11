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
/** Prove authored GIMP masks use existing canvas operations and retain native coverage ownership. */
import {
  decodeMaskPixels,
  encodeMaskPixels,
} from '../../../integrations/shared/CanvasMaskPixels.js';
import { NativeMaskFixture, nativeMaskConnection } from '../support/native_masks.js';

test('creates, edits, captures and releases a native mask through the common client/server contract', async () => {
  const native = new NativeMaskFixture();
  const f = nativeMaskConnection(native);
  try {
    const context = await f.client.execute({ type: 'context' });
    if (context.kind !== 'context' || !context.document) throw new Error('Missing GIMP document.');
    expect(context.supported).toBe(true);
    expect(context.observesChanges).toBe(false);
    const created = await f.client.execute({
      type: 'create',
      document: context.document,
      name: '<literal region>',
      source: 'selection',
      pixels: null,
    });
    if (created.kind !== 'created') throw new Error('The GIMP mask was not created.');
    expect(created.mask.canvasId).toBe('gimp');
    expect(await f.client.execute({ type: 'activate', mask: created.mask })).toEqual({
      kind: 'activated',
    });
    expect(native.selected).toBe(Number(created.mask.id));
    const result = await f.client.execute({
      type: 'capture',
      masks: [created.mask],
      maximumSize: 0,
    });
    if (result.kind !== 'captured' || !result.snapshots[0])
      throw new Error('Missing captured coverage.');
    expect([...decodeMaskPixels(result.snapshots[0].pixels)]).toEqual(Array<number>(48).fill(127));
    expect(native.snapshots.size).toBe(0);
    expect(await f.client.execute({ type: 'release', mask: created.mask })).toEqual({
      kind: 'activated',
    });
    expect(native.selected).toBe(null);
    expect(native.channels.size).toBe(1);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});

test('captures multiple imported and blank masks coherently without changing the active native mask', async () => {
  const native = new NativeMaskFixture();
  const f = nativeMaskConnection(native);
  try {
    const context = await f.client.execute({ type: 'context' });
    if (context.kind !== 'context' || !context.document) throw new Error('Missing GIMP document.');
    const pixels = encodeMaskPixels(
      8,
      6,
      new Uint8Array(48).map((_, index) => index),
    );
    const first = await f.client.execute({
      type: 'create',
      document: context.document,
      source: 'pixels',
      pixels,
      name: 'Imported region',
    });
    const second = await f.client.execute({
      type: 'create',
      document: context.document,
      source: 'blank',
      pixels: null,
      name: 'Blank region',
    });
    if (first.kind !== 'created' || second.kind !== 'created') throw new Error('Missing masks.');
    await f.client.execute({ type: 'activate', mask: first.mask });
    const captured = await f.client.execute({
      type: 'capture',
      masks: [first.mask, second.mask],
      maximumSize: 0,
    });
    if (captured.kind !== 'captured') throw new Error('Missing coherent capture.');
    expect(captured.snapshots[0]?.pixels).toEqual(pixels);
    expect(captured.snapshots[0]?.revision).toBe(captured.snapshots[1]?.revision);
    expect(native.selected).toBe(Number(first.mask.id));
    expect(native.commands.filter((command) => command.type === 'snapshot.begin')).toHaveLength(1);
    expect(native.snapshots.size).toBe(0);
  } finally {
    f.dispose();
  }
});

test('refuses stale documents, foreign canvases and malformed channel identities before native editing', async () => {
  const native = new NativeMaskFixture();
  const f = nativeMaskConnection(native);
  try {
    const context = await f.client.execute({ type: 'context' });
    if (context.kind !== 'context' || !context.document) throw new Error('Missing GIMP document.');
    const created = await f.client.execute({
      type: 'create',
      document: context.document,
      source: 'blank',
      pixels: null,
      name: 'Region',
    });
    if (created.kind !== 'created') throw new Error('Missing mask.');
    for (const mask of [
      { ...created.mask, canvasId: 'photoshop' },
      { ...created.mask, id: '050' },
      { ...created.mask, document: { ...created.mask.document, geometryRevision: 'stale' } },
    ]) {
      expect(await f.client.execute({ type: 'activate', mask })).toMatchObject({
        kind: 'failure',
        status: 'stale',
      });
    }
    expect(native.commands.some((command) => command.type === 'channel.select')).toBe(false);
    native.channels.clear();
    expect(
      await f.client.execute({ type: 'capture', masks: [created.mask], maximumSize: 0 }),
    ).toMatchObject({ kind: 'failure', status: 'stale' });
    expect(native.snapshots.size).toBe(0);
  } finally {
    f.dispose();
  }
});

test('rejects mismatched imported coverage and oversized collections before native allocation', async () => {
  const native = new NativeMaskFixture();
  const f = nativeMaskConnection(native);
  try {
    const context = await f.client.execute({ type: 'context' });
    if (context.kind !== 'context' || !context.document) throw new Error('Missing GIMP document.');
    expect(
      await f.client.execute({
        type: 'create',
        document: context.document,
        source: 'pixels',
        name: 'Region',
        pixels: encodeMaskPixels(1, 1, new Uint8Array([255])),
      }),
    ).toMatchObject({ kind: 'failure' });
    expect(native.commands.some((command) => command.type === 'channel.create')).toBe(false);
    native.document = { ...native.document, width: 8192, height: 8192 };
    const large = await f.client.execute({ type: 'context' });
    if (large.kind !== 'context' || !large.document) throw new Error('Missing large document.');
    expect(
      await f.client.execute({
        type: 'capture',
        maximumSize: 0,
        masks: [
          { canvasId: 'gimp', id: '1', document: large.document },
          { canvasId: 'gimp', id: '2', document: large.document },
        ],
      }),
    ).toMatchObject({ kind: 'failure' });
    expect(native.commands.some((command) => command.type === 'snapshot.begin')).toBe(false);
  } finally {
    f.dispose();
  }
});
