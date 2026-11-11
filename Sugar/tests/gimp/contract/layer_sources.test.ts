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
/** Prove GIMP translates every existing layer target through one coherent native snapshot. */
import { NativeCanvasFixture, nativeCanvasConnection } from '../support/native_canvas.js';

test('captures current, explicit, and ranged layers using frozen native metadata', async () => {
  const native = new NativeCanvasFixture();
  native.layers = [
    { tattoo: 1, parentTattoo: null, depth: 0, name: 'Top', visible: true, hasMask: false },
    { tattoo: 2, parentTattoo: null, depth: 0, name: 'Group', visible: false, hasMask: false },
    {
      tattoo: 4294967295,
      parentTattoo: 2,
      depth: 1,
      name: 'Nested',
      visible: false,
      hasMask: true,
    },
    { tattoo: 4, parentTattoo: null, depth: 0, name: 'Bottom', visible: true, hasMask: false },
  ];
  native.primaryLayer = 4294967295;
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing native document.');
    const result = await f.client.sources.execute({
      type: 'capture',
      document,
      maximumSize: 4,
      targets: [
        { kind: 'current-layer' },
        { kind: 'layer', layerId: '2' },
        { kind: 'composition-from-layer', layerId: '4294967295' },
        { kind: 'composition' },
      ],
    });
    expect(result.kind).toBe('captured');
    const projections = native.commands.filter((command) => command.type === 'snapshot.projection');
    expect(projections.map((command) => command.visibility)).toEqual([
      [
        { tattoo: 1, visible: false },
        { tattoo: 2, visible: true },
        { tattoo: 4294967295, visible: true },
        { tattoo: 4, visible: false },
      ],
      [
        { tattoo: 1, visible: false },
        { tattoo: 2, visible: true },
        { tattoo: 4294967295, visible: false },
        { tattoo: 4, visible: false },
      ],
      [{ tattoo: 1, visible: false }],
      [],
    ]);
    expect(new Set(projections.map((command) => command.token)).size).toBe(1);
    expect(native.snapshots.size).toBe(0);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});

test.each([{ kind: 'current-layer' } as const, { kind: 'layer', layerId: 'unknown' } as const])(
  'refuses an unavailable layer and releases its snapshot: %o',
  async (target) => {
    const native = new NativeCanvasFixture();
    const f = nativeCanvasConnection(native);
    try {
      const { document } = await f.client.readContext();
      if (!document) throw new Error('Missing native document.');
      expect(
        await f.client.sources.execute({
          type: 'capture',
          document,
          targets: [target],
          maximumSize: 0,
        }),
      ).toMatchObject({ kind: 'failure', status: 'stale' });
      expect(native.snapshots.size).toBe(0);
    } finally {
      f.dispose();
    }
  },
);
