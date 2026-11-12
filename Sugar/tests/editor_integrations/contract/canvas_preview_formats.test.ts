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
/** Match every advertised preview encoding with a legal transfer and negotiated admission. */
import { jest } from '@jest/globals';
import type { CanvasSurfaceCommand } from '../../../integrations/contracts/CanvasSurface.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { dispatchCanvasSurface } from '../../../integrations/shared/CanvasSurfaceDispatch.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import { address, begin, capable, fixture, preview, session } from '../support/canvas_preview.js';

test.each([
  ['png', true],
  ['jpeg', true],
  ['webp', false],
])('preview begin format %s has the shared capability grammar', (format, accepted) => {
  expect(
    isCanvasMessage({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 7,
      ...begin,
      format,
    }),
  ).toBe(accepted);
});

test.each(['png', 'jpeg'] as const)(
  'a host advertising only %s admits its complete transfer and refuses the other encoding',
  async (format) => {
    const f = await fixture({ capabilities: { ...capable, previewFormats: [format] } });
    try {
      const subscription = f.host.presentation!.subscribe(() => {});
      expect(await subscription.ready).toBe('observing');
      const other = format === 'png' ? 'jpeg' : 'png';
      expect((await f.host.execute({ ...begin, format: other })).status).toBe('unavailable');
      expect(f.commands).toEqual([]);
      for (const command of [
        { ...begin, format },
        {
          type: 'surface.preview.chunk',
          address,
          revision: 4,
          token: preview.token,
          leaseRevision: begin.revision,
          offset: 0,
          data: 'AQID',
        },
        {
          type: 'surface.preview.end',
          address,
          revision: 5,
          token: preview.token,
          leaseRevision: begin.revision,
        },
        {
          type: 'surface.preview.withdraw',
          address,
          revision: 6,
          token: preview.token,
          leaseRevision: begin.revision,
        },
      ] satisfies CanvasSurfaceCommand[])
        expect((await f.host.execute(command)).status).toBe('applied');
      expect(f.commands[0]).toMatchObject({ type: 'surface.preview.begin', format });
      expect(f.commands).toHaveLength(4);
      subscription.stop();
    } finally {
      f.close();
    }
  },
);

test('server dispatch refuses a legal but unadvertised encoding before host execution', async () => {
  const execute = jest.fn(async () => ({ status: 'applied' as const, message: '' }));
  const reply = await dispatchCanvasSurface(
    {
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 7,
      ...begin,
      format: 'png',
    },
    new CanvasCancellation(),
    {
      surface: {
        capabilities: capable,
        execute,
        presentation: {
          subscribe: () => ({ ready: Promise.resolve('observing'), stop: () => {} }),
        },
      },
      context: async () => ({
        canvas: {
          id: 'native',
          label: 'Native',
          capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
        },
        document: null,
      }),
      report: (error) => {
        throw error;
      },
    },
  );
  expect(reply.status).toBe('unavailable');
  expect(execute).not.toHaveBeenCalled();
});
