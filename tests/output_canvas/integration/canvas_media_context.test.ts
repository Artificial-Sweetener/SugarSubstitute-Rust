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
/** Prove shared menus preserve preferred copy ordering and exact batch-image command identity. */
import { jest } from '@jest/globals';
import { canvasContextActions } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.js';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { BrowserCanvasMediaActions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaActions.js';
import type {
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { OutputCopyFormat } from '../../../frontend/comfyui/ui/output_saving/domain/OutputMedia.js';
import { canvasSnapshot, CanvasPortFixture } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

/** Combine real catalog identities with a renderer-neutral ready snapshot. */
function snapshot(): CanvasViewSnapshot {
  const base = canvasSnapshot('media-context');
  const catalog = canvasCatalogFixture(1, 1, undefined, 'media-context');
  const item = catalog.entries[0]!.items[0]!;
  return {
    ...base,
    grid: catalog.entries,
    selected: item,
    view: { ...base.view, selection: { sourceId: item.provenance.sourceId, batchIndex: 0 } },
    resource: { ...base.resource, state: 'ready' },
    mediaActions: { preferredCopyFormat: 'png', fileAction: 'reveal' },
  };
}

afterEach(() => {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  document.body.replaceChildren();
});

test('presents preferred Copy and the explicit alternate format with local reveal or remote download', () => {
  const source = snapshot();
  const send = jest.fn<(command: CanvasCommand) => void>();
  const caps = { schema: 1 as const, commands: ['copy-media', 'file-media'] as const };
  const local = canvasContextActions(source, caps, send);
  expect(local.map((entry) => entry.label)).toEqual(['Copy', 'Copy .jpg', 'Reveal in folder']);
  local[1]!.select?.();
  expect(send).toHaveBeenCalledWith({ type: 'copy-media', target: 'primary', format: 'jpeg' });
  const remote = canvasContextActions(
    { ...source, mediaActions: { preferredCopyFormat: 'jpeg', fileAction: 'download' } },
    caps,
    send,
  );
  expect(remote.map((entry) => entry.label)).toEqual(['Copy', 'Copy .png', 'Download']);
  remote[0]!.select?.();
  expect(send).toHaveBeenLastCalledWith({
    type: 'copy-media',
    target: 'primary',
    format: 'preferred',
  });
});

test('orders comparison before a separator and image actions in the Substitute order', () => {
  const caps = {
    schema: 1 as const,
    commands: ['comparison', 'copy-media', 'file-media'] as const,
  };
  for (const profile of [
    { preferredCopyFormat: 'png' as const, fileAction: 'reveal' as const },
    { preferredCopyFormat: 'jpeg' as const, fileAction: 'download' as const },
  ]) {
    const actions = canvasContextActions({ ...snapshot(), mediaActions: profile }, caps, jest.fn());
    expect(actions.map((entry) => (entry.separator ? 'separator' : entry.label))).toEqual([
      'Compare outputs',
      'separator',
      'Copy',
      profile.preferredCopyFormat === 'png' ? 'Copy .jpg' : 'Copy .png',
      profile.fileAction === 'reveal' ? 'Reveal in folder' : 'Download',
    ]);
    const compareOnly = canvasContextActions(
      snapshot(),
      { schema: 1, commands: ['comparison'] },
      jest.fn(),
    );
    expect(compareOnly).toHaveLength(1);
    expect(compareOnly[0]?.separator).toBeUndefined();
  }
});

test('pins a batch context command to the clicked image without changing the displayed selection', async () => {
  const source = snapshot();
  const item = source.grid[1]!.items[0]!;
  const target = { sourceId: item.provenance.sourceId, itemId: item.id };
  const port = new CanvasPortFixture({ ...source, view: { ...source.view, mode: 'overview' } }, [
    'copy-media',
    'file-media',
  ]);
  const root = document.createElement('div'),
    tile = document.createElement('button');
  root.append(tile);
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    document.createElement('div'),
    port,
    jest.fn(),
    root,
    () => target,
  );
  tile.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  const copy = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
    (entry) => entry.textContent === 'Copy .jpg',
  )!;
  copy.click();
  await Promise.resolve();
  expect(port.requests[0]?.command).toEqual({ type: 'copy-media', target, format: 'jpeg' });
  expect(port.read().view.selection).toEqual(source.view.selection);
  mounted.dispose();
});

test('media owner rejects a replaced batch identity and cancels an older copy intent', async () => {
  let current = snapshot();
  const first = current.grid[0]!.items[0]!,
    other = current.grid[1]!.items[0]!;
  const copy = jest
    .fn<
      (
        item: WorkflowOutputItem,
        accepts: () => boolean,
        format: OutputCopyFormat,
      ) => Promise<boolean>
    >()
    .mockResolvedValue(true);
  const owner = new BrowserCanvasMediaActions({ read: () => current, accepts: () => true, copy });
  const target = { sourceId: other.provenance.sourceId, itemId: other.id };
  expect(await owner.run({ type: 'copy-media', target, format: 'jpeg' })).toBe(true);
  expect(copy.mock.calls[0]?.[0]).toBe(other);
  const qualifies = copy.mock.calls[0]![1];
  expect(qualifies()).toBe(true);
  await owner.run({ type: 'copy-media', target: 'primary', format: 'png' });
  expect(qualifies()).toBe(false);
  current = { ...current, grid: [], selected: first };
  expect(await owner.run({ type: 'copy-media', target, format: 'jpeg' })).toBe(false);
  expect(copy).toHaveBeenCalledTimes(2);
  owner.dispose();
});

test('disables file-backed actions while a live preview or replacement image is displayed', () => {
  const source = snapshot();
  const caps = { schema: 1 as const, commands: ['copy-media', 'file-media'] as const };
  for (const current of [
    { ...source, preview: true },
    { ...source, resource: { ...source.resource, state: 'loading' as const } },
  ])
    expect(canvasContextActions(current, caps, jest.fn()).every((entry) => entry.disabled)).toBe(
      true,
    );
});

test('aborts pending transport when the originating image retires or its attachment is disposed', async () => {
  let current = snapshot();
  const signals: AbortSignal[] = [];
  const owner = new BrowserCanvasMediaActions({
    read: () => current,
    accepts: () => true,
    copy: (_item, _accepts, _format, signal) => {
      signals.push(signal);
      return new Promise((_resolve, reject) => {
        signal.addEventListener('abort', () => reject(new DOMException('Canceled', 'AbortError')), {
          once: true,
        });
      });
    },
  });
  const first = owner.run({ type: 'copy-media', target: 'primary', format: 'png' });
  current = { ...current, preview: true };
  owner.reconcile();
  expect(signals[0]?.aborted).toBe(true);
  expect(await first).toBe(false);
  current = snapshot();
  const second = owner.run({ type: 'copy-media', target: 'primary', format: 'jpeg' });
  owner.dispose();
  expect(signals[1]?.aborted).toBe(true);
  expect(await second).toBe(false);
});
