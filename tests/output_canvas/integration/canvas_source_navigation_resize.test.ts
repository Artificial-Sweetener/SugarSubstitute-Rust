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
/** Prove contextual source controls adapt to late content and workspace resizing without oscillation. */

import { jest } from '@jest/globals';
import { createApp, h, nextTick, shallowRef } from 'vue';
import { CanvasNavigationBar } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasNavigationBar.js';
import type {
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';
import { ensureCanvasOverlayStyles } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayStyles.js';
import { ensureBrowserCanvasStyles } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasStyles.js';

/** Deliver only explicit resize notifications from the browser boundary. */
class NavigationResizeObserver implements ResizeObserver {
  static instances: NavigationResizeObserver[] = [];
  readonly targets = new Set<Element>();
  disconnected = false;

  /** Retain the browser callback for deterministic viewport changes. */
  constructor(private readonly callback: ResizeObserverCallback) {
    NavigationResizeObserver.instances.push(this);
  }

  /** Observe a concrete mounted navigation measurement target. */
  observe(target: Element): void {
    this.targets.add(target);
  }

  /** Retire a replaced tab strip. */
  unobserve(target: Element): void {
    this.targets.delete(target);
  }

  /** Release the disposed overlay's measurement subscription. */
  disconnect(): void {
    this.disconnected = true;
    this.targets.clear();
  }

  /** Notify the component after one settled viewport change. */
  notify(): void {
    this.callback([], this);
  }
}

test('keeps tab geometry and identity across grid/detail selection with browser styles mounted', async () => {
  const initial = canvasSnapshot('geometry');
  const snapshot = shallowRef<CanvasViewSnapshot>({
    ...initial,
    view: { ...initial.view, mode: 'overview' },
  });
  const port = new CanvasPortFixture(initial);
  const host = document.createElement('section');
  host.className = 'sc-output-overlay sc-output-overview-mode';
  document.body.append(host);
  ensureCanvasOverlayStyles(document);
  ensureBrowserCanvasStyles(document);
  const app = createApp({
    setup: () => () =>
      h(CanvasNavigationBar, {
        snapshot: snapshot.value,
        capabilities: port.capabilities,
        send: () => {},
        chooseRoute: () => {},
      }),
  });
  try {
    app.mount(host);
    const tabs = host.querySelector<HTMLElement>('.sc-output-tabs')!;
    const source = tabs.querySelector('button')!;
    expect(getComputedStyle(tabs).margin).toBe('0px');
    expect(getComputedStyle(source).height).toBe('30px');
    snapshot.value = { ...initial, view: { ...initial.view, mode: 'image' } };
    host.classList.remove('sc-output-overview-mode');
    await nextTick();
    expect(host.querySelector('.sc-output-tabs')).toBe(tabs);
    expect(tabs.querySelector('button')).toBe(source);
    expect(getComputedStyle(tabs).margin).toBe('0px');
    expect(getComputedStyle(source).height).toBe('30px');
    const batch = host.querySelector('button[aria-label="Output image"]')!;
    expect(getComputedStyle(batch).textOverflow).toBe('clip');
  } finally {
    app.unmount();
    host.remove();
    document.getElementById('sugarcubes-output-overlay-styles')?.remove();
    document.getElementById('sugarcubes-output-browser-styles')?.remove();
  }
});

test('collapses late source tabs into a usable chooser and restores tabs after widening', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  Object.defineProperty(globalThis, 'ResizeObserver', {
    configurable: true,
    value: NavigationResizeObserver,
  });
  NavigationResizeObserver.instances = [];
  let width = 600;
  jest.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => width);
  jest.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.getAttribute('role') === 'tablist' ? 400 : 0;
  });
  const initial = canvasSnapshot('resize');
  const snapshot = shallowRef<CanvasViewSnapshot>({
    ...initial,
    sources: initial.sources.slice(0, 1),
  });
  const port = new CanvasPortFixture(initial);
  const send = jest.fn<(command: CanvasCommand) => void>();
  const host = document.createElement('section');
  document.body.append(host);
  const app = createApp({
    setup: () => () =>
      h(CanvasNavigationBar, {
        snapshot: snapshot.value,
        capabilities: port.capabilities,
        send,
        chooseRoute: () => {},
      }),
  });
  try {
    app.mount(host);
    expect(host.querySelector('[role="tablist"]')).toBeNull();
    snapshot.value = initial;
    await nextTick();
    const observer = NavigationResizeObserver.instances[0]!;
    expect(observer.targets.size).toBe(3);
    expect(observer.targets.has(host.querySelector('.sc-output-tabstrip')!)).toBe(true);
    width = 200;
    observer.notify();
    await nextTick();
    const picker = host.querySelector<HTMLButtonElement>('button[aria-label="Output source"]')!;
    expect(picker).not.toBeNull();
    observer.notify();
    await nextTick();
    expect(host.querySelector('button[aria-label="Output source"]')).toBe(picker);
    picker.click();
    await nextTick();
    host.ownerDocument.querySelectorAll<HTMLButtonElement>('[role="option"]')[1]!.click();
    expect(send).toHaveBeenCalledWith({
      type: 'select-source',
      selection: {
        sourceId: initial.sources[1]!.id,
        batchIndex: initial.view.selection!.batchIndex,
      },
    });
    width = 600;
    observer.notify();
    await nextTick();
    expect(host.querySelector('button[aria-label="Output source"]')).toBeNull();
    expect(host.querySelector('[role="tablist"]')?.getAttribute('aria-hidden')).not.toBe('true');
    expect(host.querySelector('[role="tab"][tabindex="0"]')).not.toBeNull();
  } finally {
    app.unmount();
    host.remove();
    jest.restoreAllMocks();
    if (descriptor) Object.defineProperty(globalThis, 'ResizeObserver', descriptor);
    else Reflect.deleteProperty(globalThis, 'ResizeObserver');
  }
  expect(NavigationResizeObserver.instances[0]?.disconnected).toBe(true);
});
