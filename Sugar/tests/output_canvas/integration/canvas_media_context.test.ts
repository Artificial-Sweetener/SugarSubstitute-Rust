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
import {
  CanvasMediaActions,
  type CanvasPhotoshopLayer,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasMediaActions.js';
import type {
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { OutputCopyFormat } from '../../../frontend/comfyui/ui/output_saving/domain/OutputMedia.js';
import {
  canvasOperationScope,
  canvasSnapshot,
  CanvasPortFixture,
} from '../support/CanvasPortFixture.js';
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
    mediaActions: { preferredCopyFormat: 'png', fileAction: 'reveal', photoshop: true },
  };
}

afterEach(() => {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  document.body.replaceChildren();
});

const allCommands = [
  'comparison',
  'copy-media',
  'file-media',
  'photoshop-media',
  'photoshop-all-media',
] as const;

/** Name each entry, writing separators as such. */
function labels(entries: readonly { label: string; separator?: boolean }[]): string[] {
  return entries.map((entry) => (entry.separator ? 'separator' : entry.label));
}

test('presents the Substitute Output menu in its order with local reveal or remote download', () => {
  const caps = { schema: 1 as const, commands: allCommands };
  expect(labels(canvasContextActions(snapshot(), caps, jest.fn()))).toEqual([
    'Compare outputs',
    'separator',
    'Copy',
    'Open in Photoshop',
    'Open All in Photoshop',
    'Reveal in File Manager',
  ]);
  const remote = canvasContextActions(
    {
      ...snapshot(),
      mediaActions: { preferredCopyFormat: 'jpeg', fileAction: 'download', photoshop: false },
    },
    caps,
    jest.fn(),
  );
  expect(labels(remote)).toEqual(['Compare outputs', 'separator', 'Copy', 'Download']);
  const compareOnly = canvasContextActions(
    snapshot(),
    { schema: 1, commands: ['comparison'] },
    jest.fn(),
  );
  expect(labels(compareOnly)).toEqual(['Compare outputs']);
});

test('sends Copy in the preferred format and the Photoshop actions for their outputs', () => {
  const send = jest.fn<(command: CanvasCommand) => void>();
  const entries = canvasContextActions(snapshot(), { schema: 1, commands: allCommands }, send);
  for (const label of [
    'Copy',
    'Open in Photoshop',
    'Open All in Photoshop',
    'Reveal in File Manager',
  ])
    entries.find((entry) => entry.label === label)?.select?.();
  expect(send.mock.calls.map(([command]) => command)).toEqual([
    { type: 'copy-media', target: 'primary', format: 'preferred' },
    { type: 'photoshop-media', target: 'primary' },
    { type: 'photoshop-all-media' },
    { type: 'file-media', target: 'primary' },
  ]);
});

test('the overview offers a menu only on an image or while comparing, and never Open All', () => {
  const caps = { schema: 1 as const, commands: allCommands };
  const source = snapshot();
  const overview = { ...source, view: { ...source.view, mode: 'overview' as const } };
  expect(canvasContextActions(overview, caps, jest.fn())).toEqual([]);
  const item = source.grid[1]!.items[0]!;
  const tile = { sourceId: item.provenance.sourceId, itemId: item.id };
  const onTile = canvasContextActions(overview, caps, jest.fn(), tile);
  expect(labels(onTile)).toEqual(['Copy', 'Open in Photoshop', 'Reveal in File Manager']);
  expect(onTile.every((entry) => !entry.disabled)).toBe(true);
  const comparing = {
    ...overview,
    view: {
      ...overview.view,
      comparison: { secondary: null, orientation: 'vertical' as const, split: 0.5 },
    },
  };
  const compared = canvasContextActions(comparing, caps, jest.fn());
  expect(labels(compared)[0]).toBe('Compare outputs');
  expect(compared[0]?.checked).toBe(true);
});

test('video offers neither comparison nor Photoshop', () => {
  const source = snapshot();
  const video = { ...source.selected!, mediaKind: 'video' as const };
  const entries = canvasContextActions(
    { ...source, selected: video },
    { schema: 1, commands: allCommands },
    jest.fn(),
  );
  expect(labels(entries)).toEqual(['Copy', 'Reveal in File Manager']);
});

test('opens no menu at all in the overview away from an image', () => {
  const source = snapshot();
  const port = new CanvasPortFixture({ ...source, view: { ...source.view, mode: 'overview' } }, [
    ...allCommands,
  ]);
  const root = document.createElement('div');
  document.body.append(root);
  const mounted = mountCanvasOverlay(document.createElement('div'), port, jest.fn(), root);
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  root.dispatchEvent(event);
  expect(document.querySelector('[role="menu"]')).toBeNull();
  mounted.dispose();
});

test('Open All pins the exact image set and names each layer by source and position', async () => {
  let current = snapshot();
  const photoshop = jest
    .fn<
      (
        document: string,
        layers: readonly CanvasPhotoshopLayer[],
        accepts: () => boolean,
      ) => Promise<boolean>
    >()
    .mockResolvedValue(true);
  const owner = new CanvasMediaActions({
    // Image detail publishes no grid; Open All still reaches every image of the result.
    read: () => ({ ...current, grid: [] }),
    entries: () => current.grid,
    accepts: () => true,
    photoshop,
  });
  expect(owner.commands()).toEqual(['photoshop-media', 'photoshop-all-media']);
  expect(await owner.run({ type: 'photoshop-all-media' }, canvasOperationScope())).toBe(true);
  const [name, layers, accepts] = photoshop.mock.calls[0]!;
  const items = current.grid.flatMap((entry) => entry.items);
  expect(layers.map((layer) => layer.item)).toEqual(items);
  expect(layers.map((layer) => layer.name)).toEqual(
    items.map(
      (item) =>
        `${current.grid.find((entry) => entry.source.id === item.provenance.sourceId)!.source.label} ${item.batchIndex + 1}`,
    ),
  );
  expect(name).toBe(items[0]!.file.filename.replace(/\.[^.]*$/, ''));
  expect(accepts()).toBe(true);
  current = { ...current, grid: current.grid.slice(1) };
  expect(accepts()).toBe(false);
  current = { ...current, grid: [] };
  expect(await owner.run({ type: 'photoshop-all-media' }, canvasOperationScope())).toBe(false);
  expect(photoshop).toHaveBeenCalledTimes(1);
  owner.dispose();
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
    (entry) => entry.textContent === 'Copy',
  )!;
  copy.click();
  await Promise.resolve();
  expect(port.requests[0]?.command).toEqual({ type: 'copy-media', target, format: 'preferred' });
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
  const owner = new CanvasMediaActions({ read: () => current, accepts: () => true, copy });
  const target = { sourceId: other.provenance.sourceId, itemId: other.id };
  expect(
    await owner.run({ type: 'copy-media', target, format: 'jpeg' }, canvasOperationScope()),
  ).toBe(true);
  expect(copy.mock.calls[0]?.[0]).toBe(other);
  const qualifies = copy.mock.calls[0]![1];
  expect(qualifies()).toBe(true);
  await owner.run({ type: 'copy-media', target: 'primary', format: 'png' }, canvasOperationScope());
  expect(qualifies()).toBe(false);
  current = { ...current, grid: [], selected: first };
  expect(
    await owner.run({ type: 'copy-media', target, format: 'jpeg' }, canvasOperationScope()),
  ).toBe(false);
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
  const owner = new CanvasMediaActions({
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
  const first = owner.run(
    { type: 'copy-media', target: 'primary', format: 'png' },
    canvasOperationScope(),
  );
  current = { ...current, preview: true };
  owner.reconcile();
  expect(signals[0]?.aborted).toBe(true);
  expect(await first).toBe(false);
  current = snapshot();
  const second = owner.run(
    { type: 'copy-media', target: 'primary', format: 'jpeg' },
    canvasOperationScope(),
  );
  owner.dispose();
  expect(signals[1]?.aborted).toBe(true);
  expect(await second).toBe(false);
});

test('aborts a pending transfer when its originating surface command is superseded', async () => {
  const controller = new AbortController();
  const transfers: AbortSignal[] = [];
  const owner = new CanvasMediaActions({
    read: snapshot,
    accepts: () => true,
    copy: (_item, _accepts, _format, signal) => {
      transfers.push(signal);
      return new Promise((_resolve, reject) => {
        signal.addEventListener('abort', () => reject(new DOMException('Canceled', 'AbortError')));
      });
    },
  });
  const pending = owner.run(
    { type: 'copy-media', target: 'primary', format: 'png' },
    { signal: controller.signal, current: () => !controller.signal.aborted },
  );
  controller.abort();
  expect(transfers[0]?.aborted).toBe(true);
  expect(await pending).toBe(false);
  owner.dispose();
});
