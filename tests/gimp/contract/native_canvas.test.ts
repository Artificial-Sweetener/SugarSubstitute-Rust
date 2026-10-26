/** @jest-environment node */
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
/** Prove public canvas semantics against the GIMP adapter without a live editor in CI. */
import { decodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import {
  NativeCanvasFixture,
  nativeCanvasConnection,
  nativeFixturePng,
} from '../support/native_canvas.js';

test('publishes the actual GIMP capabilities and capture-only source contract', async () => {
  const native = new NativeCanvasFixture();
  const f = nativeCanvasConnection(native);
  try {
    const context = await f.client.readContext();
    expect(context.canvas.id).toBe('gimp');
    expect(context.document?.id).toBe('fixture-host:7');
    expect(context.canvas.capabilities).toEqual({
      displayOutput: false,
      insertImageLayer: true,
      positionImageLayer: true,
    });
    expect(await f.client.sources.execute({ type: 'context' })).toMatchObject({
      supported: true,
      observesChanges: false,
      targets: ['selection', 'current-layer', 'layer', 'composition', 'composition-from-layer'],
    });
  } finally {
    f.dispose();
  }
});

test('uses shared intrinsic-image placement for centered and explicitly positioned output', async () => {
  const native = new NativeCanvasFixture();
  native.document = { ...native.document, width: 2, height: 1 };
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    const request = {
      document,
      image: nativeFixturePng,
      layerName: '<literal layer>',
      position: null,
    };
    expect(await f.client.insertImage(request)).toMatchObject({
      status: 'inserted',
      left: -1,
      top: -1,
      width: 3,
      height: 2,
    });
    expect(
      await f.client.insertImage({ ...request, position: { left: -17, top: 23 } }),
    ).toMatchObject({ status: 'inserted', left: -17, top: 23 });
    expect(native.commands.filter((command) => command.type === 'image.insert')).toHaveLength(2);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});

test('refuses stale document geometry and unsupported native insertion without invoking a mutation', async () => {
  const native = new NativeCanvasFixture();
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    const request = { document, image: nativeFixturePng, layerName: 'Result', position: null };
    native.document = { ...native.document, width: 9 };
    expect(await f.client.insertImage(request)).toMatchObject({ status: 'stale' });
    native.rgb = false;
    expect(await f.client.insertImage(request)).toMatchObject({ status: 'unsupported' });
    expect(native.commands.some((command) => command.type === 'image.insert')).toBe(false);
  } finally {
    f.dispose();
  }
});

test('shares one frozen revision across composition and selection then releases native resources', async () => {
  const native = new NativeCanvasFixture();
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    const result = await f.client.sources.execute({
      type: 'capture',
      document,
      targets: [{ kind: 'composition' }, { kind: 'selection' }],
      maximumSize: 4,
    });
    if (result.kind !== 'captured' || !result.snapshots[0] || !result.snapshots[1])
      throw new Error('Missing source batch.');
    expect(result.snapshots[0].revision).toBe(result.snapshots[1].revision);
    expect([...decodeCanvasRaster(result.snapshots[0].pixels)]).toEqual(
      Array<number>(4 * 3 * 4).fill(7),
    );
    expect([...decodeCanvasRaster(result.snapshots[1].pixels)]).toEqual(
      Array<number>(4 * 3).fill(255),
    );
    expect(native.snapshots.size).toBe(0);
    expect(native.commands.filter((command) => command.type === 'snapshot.begin')).toHaveLength(1);
  } finally {
    f.dispose();
  }
});

test('admits the total source collection budget before freezing native contents', async () => {
  const native = new NativeCanvasFixture();
  native.document = { ...native.document, width: 4096, height: 4096 };
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    const result = await f.client.sources.execute({
      type: 'capture',
      document,
      targets: [{ kind: 'composition' }, { kind: 'selection' }],
      maximumSize: 0,
    });
    expect(result).toMatchObject({ kind: 'failure' });
    expect(native.commands.some((command) => command.type === 'snapshot.begin')).toBe(false);
  } finally {
    f.dispose();
  }
});

test('releases a snapshot whose native geometry differs from the admitted destination', async () => {
  const native = new NativeCanvasFixture();
  native.staleSnapshot = true;
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    expect(
      await f.client.sources.execute({
        type: 'capture',
        document,
        targets: [{ kind: 'composition' }],
        maximumSize: 0,
      }),
    ).toMatchObject({ kind: 'failure', status: 'stale' });
    expect(native.snapshots.size).toBe(0);
  } finally {
    f.dispose();
  }
});

test('retains failed releases and retries them before admitting another native snapshot', async () => {
  const native = new NativeCanvasFixture();
  native.releaseFailures = 1;
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing fixture document.');
    const query = {
      type: 'capture',
      document,
      targets: [{ kind: 'composition' }],
      maximumSize: 0,
    } as const;
    expect(await f.client.sources.execute(query)).toMatchObject({ kind: 'failure' });
    expect(native.snapshots.size).toBe(1);
    expect(await f.client.sources.execute(query)).toMatchObject({ kind: 'captured' });
    expect(native.snapshots.size).toBe(0);
  } finally {
    f.dispose();
  }
});
