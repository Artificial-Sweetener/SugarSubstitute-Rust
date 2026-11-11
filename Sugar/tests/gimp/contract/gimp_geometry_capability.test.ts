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
/** Prove GIMP sizing uses the same negotiated geometry transport as other canvases. */
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { NativeCanvasFixture, nativeCanvasConnection } from '../support/native_canvas.js';

test('reads selection geometry and resizes the captured GIMP canvas through the shared client', async () => {
  const native = new NativeCanvasFixture();
  const f = nativeCanvasConnection(native);
  try {
    const context = await f.client.readContext();
    expect(context.canvas.capabilities).toMatchObject({ geometry: true });
    const geometry = await f.client.geometry.execute({ type: 'context' });
    if (geometry.kind !== 'context' || !geometry.document) throw new Error('Missing geometry.');
    expect(geometry).toMatchObject({
      supported: true,
      resize: true,
      selectionSupported: true,
      selection: { left: 1, top: 2, width: 3, height: 2 },
    });
    const resized = await f.client.geometry.execute({
      type: 'resize',
      document: geometry.document,
      size: { width: 1200, height: 800 },
    });
    expect(resized).toMatchObject({ kind: 'resized', document: { width: 1200, height: 800 } });
    expect(native.commands.filter((command) => command.type === 'image.resize')).toHaveLength(1);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});

test('refuses stale geometry and cancellation without mutating the native document', async () => {
  const native = new NativeCanvasFixture();
  const f = nativeCanvasConnection(native);
  try {
    const { document } = await f.client.readContext();
    if (!document) throw new Error('Missing document.');
    native.document = { ...native.document, width: 99 };
    expect(
      await f.client.geometry.execute({
        type: 'resize',
        document,
        size: { width: 200, height: 100 },
      }),
    ).toMatchObject({ kind: 'failure', status: 'stale' });
    const cancellation = new CanvasCancellation();
    cancellation.cancel();
    expect(
      await f.client.geometry.execute(
        { type: 'resize', document, size: { width: 200, height: 100 } },
        cancellation,
      ),
    ).toMatchObject({ kind: 'failure', status: 'cancelled' });
    expect(native.commands.some((command) => command.type === 'image.resize')).toBe(false);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});
