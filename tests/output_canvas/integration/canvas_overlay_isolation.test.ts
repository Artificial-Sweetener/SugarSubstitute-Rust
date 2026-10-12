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

test('mounts browser and host-backed contract roots without Comfy or browser image allocation', async () => {
  const browser = new CanvasPortFixture(canvasSnapshot('browser'));
  const host = new CanvasPortFixture({
    ...canvasSnapshot('host'),
    zoom: {
      mode: 'native',
      physicalScale: 1,
      dimensions: { width: 16384, height: 8192 },
      secondaryPhysicalScale: null,
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
    { type: 'select', selection: { sourceId: 'source:42:23', batchIndex: 2 } },
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
    ['select', 'fit'],
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
