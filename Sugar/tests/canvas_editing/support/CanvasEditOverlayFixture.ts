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
/** Mount the editing chrome over a bare canvas attachment driven by a fake editing host. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasEditOverlay } from '../../../frontend/comfyui/ui/canvas_editing/presentation/CanvasEditOverlayRuntime.js';
import type { CanvasEditState } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasEditHostFixture, editState, settle } from './CanvasEditHostFixture.js';

const mounted = new Set<{ dispose(): void }>();

/** Dispose every chrome mounted by a test, retiring its document keyboard listeners. */
export function disposeEditing(): void {
  for (const overlay of mounted) overlay.dispose();
  mounted.clear();
}

/** The canvas viewport's client box; the chrome layer shares it. */
export const VIEWPORT = { left: 100, top: 50, width: 800, height: 600 } as const;

export function clientRect(box: {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}): DOMRect {
  return {
    x: box.left,
    y: box.top,
    left: box.left,
    top: box.top,
    right: box.left + box.width,
    bottom: box.top + box.height,
    width: box.width,
    height: box.height,
    toJSON: () => ({}),
  };
}

/** Flush command replies, Vue renders and post-render placement. */
export async function flush(): Promise<void> {
  await settle();
  await nextTick();
  await settle();
}

/**
 * Build attachment, viewport and chrome layer like the native attachment does, and attach a
 * fake host. Every rendered toolbar measures 200 by 36 CSS pixels. `obstacles` stands in for
 * the host's route tabs, as client rectangles.
 */
export async function mountEditing(
  state: CanvasEditState = editState(),
  obstacles: () => readonly DOMRectReadOnly[] = () => [],
) {
  const host = new CanvasEditHostFixture(state);
  const scope = document.createElement('section');
  const viewport = document.createElement('div');
  viewport.setAttribute('data-sc-canvas-geometry', '');
  const root = document.createElement('div');
  scope.append(viewport, root);
  document.body.append(scope);
  jest.spyOn(root, 'getBoundingClientRect').mockReturnValue(clientRect(VIEWPORT));
  jest.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(200);
  jest.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(36);
  const errors: unknown[] = [];
  const overlay = mountCanvasEditOverlay({
    root,
    scope,
    region: viewport,
    geometry: () => ({
      origin: { x: VIEWPORT.left, y: VIEWPORT.top },
      viewport: { width: VIEWPORT.width, height: VIEWPORT.height },
      physicalPixelsPerCssPixel: 1,
    }),
    obstacles,
    maskColor: (layer) => (layer === 'mask-layer-1' ? '#33cc99' : null),
    report: (error) => errors.push(error),
  });
  mounted.add(overlay);
  overlay.attach(host);
  await flush();
  const query = <E extends Element = HTMLElement>(selector: string) =>
    root.querySelector<E & Element>(selector);
  return {
    host,
    scope,
    viewport,
    root,
    mounted: overlay,
    errors,
    query,
    /** Click one control by its data attribute selector. */
    async click(selector: string): Promise<void> {
      const element = root.querySelector<HTMLElement>(selector);
      if (!element) throw new Error(`Missing ${selector}`);
      element.click();
      await flush();
    },
    /** Push a host state change and wait for the chrome to render it. */
    async push(patch: Partial<CanvasEditState>): Promise<void> {
      host.push(patch);
      await flush();
    },
  };
}

/** Dispatch a document keyboard event from a target and report whether the canvas took it. */
export function press(
  target: EventTarget,
  key: string,
  options: KeyboardEventInit = {},
  type: 'keydown' | 'keyup' = 'keydown',
): boolean {
  const event = new KeyboardEvent(type, { key, bubbles: true, cancelable: true, ...options });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}
