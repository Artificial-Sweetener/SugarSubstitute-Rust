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
/** Prove reusable overlays remain isolated from images, Comfy, and retired attachments. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';
import { canvasContextActions } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

/** Allocate an explicitly owned test root. */
function root(): HTMLElement {
  const element = document.createElement('section');
  document.body.append(element);
  return element;
}

/** Resolve controls by accessible text rather than source order. */
function button(element: HTMLElement, text: string): HTMLElement {
  const found = [...element.querySelectorAll('button')].find(
    (candidate) => candidate.textContent === text,
  );
  if (!found) {
    element.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const item = [
      ...document.querySelectorAll<HTMLElement>('[role="menuitem"],[role="menuitemcheckbox"]'),
    ].find((candidate) => candidate.textContent === text);
    if (!item) throw new Error(`Missing test control: ${text}`);
    return item;
  }
  return found;
}

/** Exercise an actual anchored row picker without mutating a private selection state. */
async function choose(element: HTMLElement, label: string, text: string): Promise<void> {
  const anchor = element.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
  anchor.click();
  await nextTick();
  const list = element.ownerDocument.querySelector(
    `[role="listbox"][aria-label="${label} choices"]`,
  )!;
  const row = [...list.querySelectorAll<HTMLButtonElement>('[role="option"]')].find(
    (item) => item.textContent === text,
  );
  if (!row) throw new Error(`Missing picker row: ${text}`);
  row.click();
  await nextTick();
}

afterEach(() => {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('shows numbered sets and source tabs without adding a canvas mode', () => {
  const port = new CanvasPortFixture({
    ...canvasSnapshot('reference-controls'),
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('[aria-label="Canvas content"]')).toBeNull();
  expect(element.querySelector('select')).toBeNull();
  expect(
    element.querySelector('[aria-label="Previous image"],[aria-label="Next image"]'),
  ).toBeNull();
  expect(element.querySelector('[aria-label="Output image"]')?.textContent).toBe('3');
  mounted.dispose();
});

test('offers only the reference comparison toggle outside comparison mode', () => {
  const port = new CanvasPortFixture(canvasSnapshot('reference-menu'));
  expect(
    canvasContextActions(port.read(), port.capabilities, jest.fn()).map((entry) => entry.label),
  ).toEqual(['Compare outputs']);
});

test('describes a reactivated preview-only source before its next frame arrives', () => {
  const initial = canvasSnapshot('waiting-preview');
  const port = new CanvasPortFixture({
    ...initial,
    sources: initial.sources.map((source) => ({ ...source, batchCount: 1, preview: true })),
    resource: {
      state: 'empty',
      message: 'Waiting for a fresh source preview.',
      canRetry: false,
      canAdmitLargeImage: false,
    },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('[aria-label="Batch position"]')).toBeNull();
  expect(element.textContent).not.toContain('/ 0');
  expect(element.querySelector('[aria-label="Next image"]')).toBeNull();
  mounted.dispose();
});

test('keeps source and comparison catalogs outside continuous camera and wipe rendering', async () => {
  const initial = canvasSnapshot('navigation-cadence');
  let labelReads = 0;
  Object.defineProperty(initial.sources[0]!, 'label', {
    get: () => {
      labelReads++;
      return 'Saved image';
    },
  });
  const port = new CanvasPortFixture({
    ...initial,
    view: {
      ...initial.view,
      comparison: { secondary: null, split: 0.5, orientation: 'vertical' },
    },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  const picker = element.querySelector('[aria-label="First output"]');
  labelReads = 0;
  for (let update = 2; update <= 100; update++) {
    const snapshot = port.read();
    port.publish({
      ...snapshot,
      revision: update,
      view: {
        ...snapshot.view,
        comparison: { secondary: null, split: update / 100, orientation: 'vertical' },
      },
      zoom: { ...snapshot.zoom, physicalScale: update / 10 },
    });
    await nextTick();
  }
  expect(labelReads).toBe(0);
  expect(element.querySelector('[aria-label="First output"]')).toBe(picker);
  expect(element.querySelector('[role="separator"]')?.getAttribute('aria-valuenow')).toBe('100');
  port.publish({
    ...port.read(),
    revision: 101,
    sources: initial.sources.map((source) => ({ ...source, label: 'Renamed' })),
  });
  await nextTick();
  expect(element.querySelector('[aria-label="First output"]')?.textContent).toBe('Renamed');
  mounted.dispose();
});

test('keeps zoom and preview changes outside the live announcement region', async () => {
  const initial = canvasSnapshot('announcements');
  const port = new CanvasPortFixture(initial);
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  const live = element.querySelector('[aria-live="polite"]')!;
  expect(live.querySelector('[aria-label="Zoom"]')).toBeNull();
  port.publish({
    ...initial,
    revision: 2,
    view: { ...initial.view, selection: { sourceId: initial.sources[1]!.id, batchIndex: 0 } },
  });
  await nextTick();
  expect(live.textContent).toBe('Detail');
  for (let update = 3; update <= 100; update++)
    port.publish({
      ...port.read(),
      revision: update,
      preview: true,
      zoom: { ...port.read().zoom, physicalScale: update / 10 },
    });
  await nextTick();
  expect(live.textContent).toBe('Detail');
  port.publish({
    ...port.read(),
    revision: 101,
    resource: { ...initial.resource, state: 'error', message: 'Image failed.' },
  });
  await nextTick();
  expect(live.textContent).toBe('Image failed.');
  mounted.dispose();
});

test('navigates source tabs by keyboard within their root while retaining the requested batch', async () => {
  const port = new CanvasPortFixture(canvasSnapshot('keyboard'));
  const other = new CanvasPortFixture(canvasSnapshot('other-root'));
  const element = root();
  const otherRoot = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  const isolated = mountCanvasOverlay(otherRoot, other, jest.fn());
  const tabs = [...element.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  expect(tabs.map((tab) => tab.tabIndex)).toEqual([0, -1]);
  tabs[0]!.focus();
  tabs[0]!.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
  await nextTick();
  expect(document.activeElement).toBe(tabs[1]);
  expect(port.requests.at(-1)?.command).toEqual({
    type: 'select-source',
    selection: { sourceId: 'source:42:23', batchIndex: 2 },
  });
  tabs[1]!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  expect(document.activeElement).toBe(tabs[0]);
  expect(other.requests).toEqual([]);
  mounted.dispose();
  isolated.dispose();
});

test('mounts browser and host-backed contract roots without Comfy or browser image allocation', async () => {
  const browser = new CanvasPortFixture(canvasSnapshot('browser'));
  const host = new CanvasPortFixture({
    ...canvasSnapshot('host'),
    zoom: {
      mode: 'native',
      physicalScale: 1,
      dimensions: { width: 16384, height: 8192 },
      secondaryPhysicalScale: null,
      secondaryDimensions: null,
    },
  });
  const browserRoot = root();
  const hostRoot = root();
  const report = jest.fn();
  const a = mountCanvasOverlay(browserRoot, browser, report);
  const b = mountCanvasOverlay(hostRoot, host, report);
  expect(document.querySelectorAll('img, canvas')).toHaveLength(0);
  expect(hostRoot.querySelector('[aria-label="Zoom"]')).toBeNull();
  button(browserRoot, 'Saved image').click();
  button(browserRoot, 'Detail').click();
  await nextTick();
  expect(host.requests).toHaveLength(0);
  expect(browser.requests.map((request) => request.command)).toEqual([
    { type: 'select-source', selection: { sourceId: 'source:7', batchIndex: 2 } },
    { type: 'select-source', selection: { sourceId: 'source:42:23', batchIndex: 2 } },
  ]);
  expect(browser.requests[0]?.address).toBe(browser.read().address);
  expect(browser.requests[0]?.expectedRevision).toBe(1);
  a.dispose();
  expect(browser.listenerCount).toBe(0);
  expect(host.listenerCount).toBe(1);
  expect(hostRoot.textContent).not.toContain('16384');
  expect(browser.releases).toHaveLength(0);
  b.dispose();
  expect(host.listenerCount).toBe(0);
  expect(report).not.toHaveBeenCalled();
});

test('renders markup-like metadata literally and hides unsupported commands', () => {
  const initial = canvasSnapshot('host');
  const port = new CanvasPortFixture(
    {
      ...initial,
      sources: initial.sources.map((source) => ({
        ...source,
        label: '<img src=x onerror=alert(1)>',
      })),
    },
    ['select-source', 'fit'],
  );
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('img, script')).toBeNull();
  expect(element.textContent).toContain('<img src=x onerror=alert(1)>');
  expect([...element.querySelectorAll('button')].map((control) => control.textContent)).toEqual([
    '<img src=x onerror=alert(1)>',
    '<img src=x onerror=alert(1)>',
  ]);
  mounted.dispose();
});

test('rejects old revisions and delivery from another attachment', async () => {
  const initial = canvasSnapshot('first');
  const port = new CanvasPortFixture(initial);
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  port.publish({
    ...initial,
    revision: 2,
    resource: { ...initial.resource, state: 'error', message: 'Current state' },
  });
  await nextTick();
  expect(element.textContent).toContain('Current state');
  port.publish({
    ...initial,
    revision: 1,
    resource: { ...initial.resource, message: 'Old state' },
  });
  port.publish({
    ...initial,
    address: { ...initial.address, windowId: windowId('other') },
    revision: 10,
    resource: { ...initial.resource, message: 'Wrong window' },
  });
  port.publish({
    ...initial,
    address: { ...initial.address, generation: 2 },
    revision: 11,
    resource: { ...initial.resource, message: 'Retired generation' },
  });
  await nextTick();
  expect(element.textContent).toContain('Current state');
  expect(element.textContent).not.toContain('Wrong window');
  expect(element.textContent).not.toContain('Retired generation');
  mounted.dispose();
});

test('delegates both on-canvas comparison choosers through the renderer-independent port', async () => {
  const initial = canvasSnapshot('comparison');
  const port = new CanvasPortFixture({
    ...initial,
    view: {
      ...initial.view,
      comparison: {
        secondary: { sourceId: 'source:42:23', batchIndex: 2 },
        orientation: 'vertical',
        split: 0.5,
      },
    },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('input,[role="toolbar"]')).toBeNull();
  await choose(element, 'First output', 'Detail');
  await choose(element, 'Comparison output', 'Saved image');
  const actions = canvasContextActions(port.read(), port.capabilities, (command) => {
    void port.dispatch({
      schema: 1,
      address: initial.address,
      requestId: port.requests.length + 1,
      expectedRevision: 1,
      command,
    });
  });
  actions.find((entry) => entry.label === 'Compare outputs')?.select?.();
  await nextTick();
  expect(port.requests.map((value) => value.command)).toEqual([
    { type: 'select', selection: { sourceId: 'source:42:23', batchIndex: 2 } },
    { type: 'select-comparison', selection: { sourceId: 'source:7', batchIndex: 2 } },
    { type: 'comparison', enabled: false },
  ]);
  mounted.dispose();
});

test('disables comparison for previews and renders only tabs for a ready single output', () => {
  const initial = canvasSnapshot('preview');
  const port = new CanvasPortFixture({
    ...initial,
    preview: true,
    sources: initial.sources.map((source) => ({ ...source, batchCount: 1 })),
    resource: { ...initial.resource, state: 'ready' },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('.sc-output-status,[role="toolbar"],input,select')).toBeNull();
  expect(element.querySelectorAll('button')).toHaveLength(2);
  expect(
    canvasContextActions(port.read(), port.capabilities, jest.fn()).find(
      (entry) => entry.label === 'Compare outputs',
    )?.disabled,
  ).toBe(true);
  mounted.dispose();
});

test('opens actions by keyboard only within the explicit interaction surface and removes the menu on teardown', () => {
  const port = new CanvasPortFixture(canvasSnapshot('scoped-menu'));
  const element = root();
  const surface = root();
  surface.append(element);
  const unrelated = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn(), surface);
  unrelated.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));
  expect(document.querySelector('[role="menu"]')).toBeNull();
  surface.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true, cancelable: true }),
  );
  expect(document.querySelector('[role="menu"]')?.getAttribute('aria-label')).toBe(
    'Output actions',
  );
  mounted.dispose();
  expect(document.querySelector('[role="menu"]')).toBeNull();
  surface.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));
  expect(document.querySelector('[role="menu"]')).toBeNull();
});

test('dismisses shared actions when clicking elsewhere on their originating canvas', () => {
  const port = new CanvasPortFixture(canvasSnapshot('outside-menu'));
  const element = root();
  const surface = root();
  const pixels = document.createElement('div');
  surface.append(element, pixels);
  const mounted = mountCanvasOverlay(element, port, jest.fn(), surface);
  surface.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  expect(document.querySelector('[role="menu"]')).not.toBeNull();
  pixels.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
  expect(document.querySelector('[role="menu"]')).toBeNull();
  mounted.dispose();
});

test('retains the context menu through camera updates without rebuilding catalog-bearing rows', async () => {
  const port = new CanvasPortFixture(canvasSnapshot('menu-cadence'));
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  element.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));
  const row = document.activeElement;
  const update = jest.spyOn(ContextMenu.prototype, 'updateEntries');
  for (let revision = 2; revision < 50; revision++)
    port.publish({
      ...port.read(),
      revision,
      zoom: { ...port.read().zoom, physicalScale: revision },
    });
  await nextTick();
  expect(document.activeElement).toBe(row);
  expect(update).not.toHaveBeenCalled();
  mounted.dispose();
});

test('disposes pending commands without updating a replacement root', async () => {
  const old = new CanvasPortFixture(canvasSnapshot('old'));
  old.delayed = true;
  const element = root();
  const report = jest.fn();
  const mounted = mountCanvasOverlay(element, old, report);
  button(element, 'Detail').click();
  mounted.dispose();
  const replacement = new CanvasPortFixture(canvasSnapshot('new'));
  const current = mountCanvasOverlay(element, replacement, report);
  old.settle(1, { status: 'error', revision: 1, message: 'Old failure' });
  old.publish({ ...old.read(), revision: 10 });
  await nextTick();
  expect(old.listenerCount).toBe(0);
  expect(element.textContent).not.toContain('Old failure');
  expect(element.querySelector('[data-canvas-view]')?.getAttribute('data-canvas-view')).toBe(
    'view:new',
  );
  current.dispose();
});

test('reports unexpected command failures with their cause and inline feedback', async () => {
  const port = new CanvasPortFixture(canvasSnapshot('failure'));
  port.failure = new Error('Native view unavailable.');
  const element = root();
  const report = jest.fn();
  const alert = jest.spyOn(window, 'alert').mockImplementation(() => undefined);
  const mounted = mountCanvasOverlay(element, port, report);
  button(element, 'Detail').click();
  await nextTick();
  await nextTick();
  expect(report).toHaveBeenCalledWith(port.failure, port.requests[0]);
  expect(element.textContent).toContain('Native view unavailable.');
  expect(alert).not.toHaveBeenCalled();
  alert.mockRestore();
  mounted.dispose();
});

test('returns root subscriptions and DOM to baseline through repeated lifecycle changes', () => {
  const port = new CanvasPortFixture(canvasSnapshot('repeated'));
  const element = root();
  for (let count = 0; count < 100; count++) {
    const mounted = mountCanvasOverlay(element, port, jest.fn());
    expect(port.listenerCount).toBe(1);
    mounted.dispose();
    mounted.dispose();
    expect(port.listenerCount).toBe(0);
    expect(element.childElementCount).toBe(0);
  }
  expect(port.releases).toHaveLength(0);
});

test('uses a numbered batch picker only for multiple images and retains exact unavailable positions', async () => {
  const initial = canvasSnapshot('batch-strip');
  const port = new CanvasPortFixture(initial);
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('[aria-label="Output image"]')?.textContent).toBe('3');
  await choose(element, 'Output image', '2');
  expect(port.requests.at(-1)?.command).toEqual({ type: 'batch', index: 1 });
  expect(
    element.querySelector('[aria-label="Previous image"],[aria-label="Next image"]'),
  ).toBeNull();
  port.publish({
    ...initial,
    revision: 2,
    view: {
      ...initial.view,
      selection: { sourceId: initial.sources[0]!.id, batchIndex: 4 },
      comparison: { secondary: null, split: 0.5, orientation: 'vertical' },
    },
  });
  await nextTick();
  expect(element.querySelector('[aria-label="First output image"]')?.textContent).toBe('5');
  mounted.dispose();
});

test('keeps actionable image failure feedback available over the simplified presentation', async () => {
  const initial = canvasSnapshot('failure-feedback');
  const port = new CanvasPortFixture({
    ...initial,
    resource: { ...initial.resource, state: 'ready' },
    secondaryResource: {
      state: 'error',
      message: 'Comparison failed.',
      canRetry: true,
      canAdmitLargeImage: false,
    },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.textContent).toContain('Comparison failed.');
  button(element, 'Retry image').click();
  await nextTick();
  expect(port.requests.at(-1)?.command).toEqual({ type: 'retry' });
  mounted.dispose();
});

test('hides lone-source chrome without adding input or output modes', async () => {
  const initial = canvasSnapshot('routes');
  const port = new CanvasPortFixture({
    ...initial,
    sources: initial.sources.slice(0, 1).map((source) => ({ ...source, batchCount: 1 })),
    comparisonAvailable: false,
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('.sc-output-tabstrip')).toBeNull();
  port.publish({
    ...port.read(),
    revision: 2,
  });
  await nextTick();
  expect(element.querySelector('[aria-label="Canvas content"]')).toBeNull();
  expect(element.querySelectorAll('[role="tab"]')).toHaveLength(0);
  mounted.dispose();
  expect(port.listenerCount).toBe(0);
});

test('shares numbered sets across sources and opens the selected source batch grid through zero', async () => {
  const initial = canvasSnapshot('numbered-sets');
  const port = new CanvasPortFixture({
    ...initial,
    sources: initial.sources.map((source, index) => ({
      ...source,
      batchCount: index === 0 ? 1 : 3,
    })),
    view: { ...initial.view, selection: { sourceId: initial.sources[0]!.id, batchIndex: 0 } },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  await choose(element, 'Output image', '0');
  expect(port.requests.at(-1)?.command).toEqual({ type: 'overview', enabled: true });
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    view: { ...port.read().view, mode: 'overview' },
  });
  await nextTick();
  expect(element.querySelector('[aria-label="Output image"]')?.textContent).toBe('0');
  await choose(element, 'Output image', '3');
  expect(port.requests.at(-1)?.command).toEqual({ type: 'batch', index: 2 });
  expect(element.querySelector('select')).toBeNull();
  mounted.dispose();
});
