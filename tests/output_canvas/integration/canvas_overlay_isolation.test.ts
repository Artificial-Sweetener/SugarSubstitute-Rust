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
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

/** Allocate an explicitly owned test root. */
function root(): HTMLElement {
  const element = document.createElement('section');
  document.body.append(element);
  return element;
}

/** Resolve controls by accessible text rather than source order. */
function button(element: HTMLElement, text: string): HTMLButtonElement {
  const found = [...element.querySelectorAll('button')].find(
    (candidate) => candidate.textContent === text,
  );
  if (!found) throw new Error(`Missing test control: ${text}`);
  return found;
}

afterEach(() => document.body.replaceChildren());

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
  const tab = element.querySelector('[role="tab"]');
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
  expect(element.querySelector('[role="tab"]')).toBe(tab);
  expect(element.querySelector('[role="separator"]')?.getAttribute('aria-valuenow')).toBe('100');
  port.publish({
    ...port.read(),
    revision: 101,
    sources: initial.sources.map((source) => ({ ...source, label: 'Renamed' })),
  });
  await nextTick();
  expect(element.querySelector('[role="tab"]')?.textContent).toBe('Renamed');
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
  expect(hostRoot.querySelector('[aria-label="Zoom"]')?.textContent).toBe('1:1');
  button(browserRoot, 'Fit').click();
  button(browserRoot, 'Detail').click();
  await nextTick();
  expect(host.requests).toHaveLength(0);
  expect(browser.requests.map((request) => request.command)).toEqual([
    { type: 'fit' },
    { type: 'select-source', selection: { sourceId: 'source:42:23', batchIndex: 2 } },
  ]);
  expect(browser.requests[0]?.address).toBe(browser.read().address);
  expect(browser.requests[0]?.expectedRevision).toBe(1);
  a.dispose();
  expect(browser.listenerCount).toBe(0);
  expect(host.listenerCount).toBe(1);
  expect(hostRoot.textContent).toContain('16384 × 8192');
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
    'Fit',
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
    resource: { ...initial.resource, message: 'Current state' },
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

test('shows an explicit chooser for comparison without a distinct second image', () => {
  const initial = canvasSnapshot('empty-comparison');
  const port = new CanvasPortFixture({
    ...initial,
    view: { ...initial.view, comparison: { secondary: null, orientation: 'vertical', split: 0.5 } },
  });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  const chooser = element.querySelector<HTMLSelectElement>('[aria-label="Comparison source"]');
  expect(chooser?.value).toBe('');
  expect(chooser?.selectedOptions[0]?.textContent).toBe('Choose an output');
  expect(button(element, 'Swap A/B').disabled).toBe(true);
  mounted.dispose();
});

test('labels unknown preview batch position and keeps comparison availability truthful', () => {
  const initial = canvasSnapshot('preview');
  const port = new CanvasPortFixture({ ...initial, preview: true });
  const element = root();
  const mounted = mountCanvasOverlay(element, port, jest.fn());
  expect(element.querySelector('[aria-label="Batch position"]')?.textContent).toBe(
    'Source preview',
  );
  expect(button(element, 'Compare').disabled).toBe(true);
  mounted.dispose();
});

test('delegates comparison activation and secondary selection as separate intents', async () => {
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
  button(element, 'Compare').click();
  const chooser = element.querySelector('select');
  if (!chooser) throw new Error('Missing comparison source chooser.');
  chooser.value = 'source:7';
  chooser.dispatchEvent(new Event('change'));
  await nextTick();
  expect(port.requests.map((value) => value.command)).toEqual([
    { type: 'comparison', enabled: false },
    { type: 'select-comparison', selection: { sourceId: 'source:7', batchIndex: 2 } },
  ]);
  mounted.dispose();
});

test('disposes pending commands without updating a replacement root', async () => {
  const old = new CanvasPortFixture(canvasSnapshot('old'));
  old.delayed = true;
  const element = root();
  const report = jest.fn();
  const mounted = mountCanvasOverlay(element, old, report);
  button(element, 'Fit').click();
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
  button(element, 'Fit').click();
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
