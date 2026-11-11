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
/** Prove preview caching is bounded and canceled capture cannot revive retired output. */
import { jest } from '@jest/globals';
import { BrowserCanvasMediaPosters } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaPosters.js';
import { mixedMediaFixture } from '../support/CanvasMixedMediaFixture.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';

test('reuses revision-qualified previews while bounding retained encoded bytes and count', async () => {
  const render = jest
    .fn<() => Promise<Blob>>()
    .mockResolvedValue(new Blob(['synthetic'], { type: 'image/webp' }));
  const posters = new BrowserCanvasMediaPosters({ render }, new MemoryBudgetRegistry());
  const item = mixedMediaFixture().items[1]!;
  const signal = new AbortController().signal;
  await posters.read(item, '/view', signal);
  await posters.read(item, '/view', signal);
  expect(render).toHaveBeenCalledTimes(1);
  for (let index = 0; index < 40; index++)
    await posters.read(
      {
        ...item,
        id: `result:poster-${index}`,
        file: { ...item.file, filename: `poster-${index}.webm` },
      },
      '/view',
      signal,
    );
  expect(posters.diagnostics().posters).toBe(24);
  expect(posters.diagnostics().posterBytes).toBeLessThanOrEqual(8 * 1024 * 1024);
  posters.dispose();
  expect(posters.diagnostics()).toEqual({ posters: 0, posterBytes: 0 });
});

test('does not retain a capture completed after cancellation', async () => {
  const controller = new AbortController();
  let finish: (blob: Blob) => void = () => {};
  const posters = new BrowserCanvasMediaPosters(
    {
      render: () =>
        new Promise<Blob>((resolve) => {
          finish = resolve;
        }),
    },
    new MemoryBudgetRegistry(),
  );
  const capture = posters.read(mixedMediaFixture().items[2]!, '/view', controller.signal);
  const rejected = expect(capture).rejects.toBeDefined();
  await Promise.resolve();
  controller.abort();
  finish(new Blob(['synthetic']));
  await rejected;
  expect(posters.diagnostics().posters).toBe(0);
});

test('shares the same artifact capture across batch positions and keeps remaining consumers alive', async () => {
  let finish: (blob: Blob) => void = () => {};
  let captureSignal: AbortSignal | undefined;
  const render = jest.fn((_item: unknown, _url: string, signal: AbortSignal) => {
    captureSignal = signal;
    return new Promise<Blob>((resolve) => {
      finish = resolve;
    });
  });
  const posters = new BrowserCanvasMediaPosters({ render }, new MemoryBudgetRegistry());
  const item = mixedMediaFixture().items[2]!;
  const first = new AbortController();
  const a = posters.read(item, '/view', first.signal);
  const b = posters.read(
    { ...item, id: 'result:another-batch-position', batchIndex: 7 },
    '/view',
    new AbortController().signal,
  );
  await Promise.resolve();
  const rejected = expect(a).rejects.toBeDefined();
  first.abort();
  expect(captureSignal?.aborted).toBe(false);
  const blob = new Blob(['synthetic']);
  finish(blob);
  await rejected;
  await expect(b).resolves.toBe(blob);
  expect(render).toHaveBeenCalledTimes(1);
  expect(posters.diagnostics().posters).toBe(1);
});
