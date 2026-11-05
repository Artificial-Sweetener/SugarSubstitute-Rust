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
/** Prove the Input/Output route in the canvas tab controls. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import type {
  CanvasRouteId,
  CanvasRoutePort,
  CanvasRouteState,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasRoute.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => document.body.replaceChildren());

const labels = { input: 'Input', output: 'Output', group: 'Canvas route' };

/** Hold a route state that tests replace like the provisioning owner does. */
function routePort(initial: CanvasRouteState | null) {
  let state = initial;
  const listeners = new Set<() => void>();
  const choose = jest.fn<(route: CanvasRouteId) => void>();
  const port: CanvasRoutePort = {
    read: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    choose,
    subscribeFocus: () => () => {},
  };
  return {
    port,
    choose,
    publish(next: CanvasRouteState | null) {
      state = next;
      for (const listener of listeners) listener();
    },
  };
}

/** Mount the overlay with a route port. */
function mount(route: CanvasRoutePort | null) {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('route')),
    jest.fn(),
    root,
    undefined,
    route,
  );
  return { root, mounted };
}

test('canvases without workflow surfaces show no route', () => {
  const { root, mounted } = mount(null);
  expect(root.querySelector('.sc-output-route')).toBeNull();
  mounted.dispose();
  const empty = routePort(null);
  const second = mount(empty.port);
  expect(second.root.querySelector('.sc-output-route')).toBeNull();
  second.mounted.dispose();
});

test('Input is disabled with its reason until the workflow has surfaces', async () => {
  const route = routePort({
    route: 'output',
    inputAvailable: false,
    reason: 'No input canvas nodes',
    labels,
  });
  const { root, mounted } = mount(route.port);
  const input = root.querySelector<HTMLButtonElement>('[data-route="input"]');
  const output = root.querySelector<HTMLButtonElement>('[data-route="output"]');
  expect([input?.textContent, output?.textContent]).toEqual(['Input', 'Output']);
  expect(input?.disabled).toBe(true);
  expect(input?.title).toBe('No input canvas nodes');
  expect(output?.getAttribute('aria-selected')).toBe('true');
  input?.click();
  expect(route.choose).not.toHaveBeenCalled();
  route.publish({ route: 'output', inputAvailable: true, reason: '', labels });
  await nextTick();
  input?.click();
  expect(route.choose).toHaveBeenCalledWith('input');
  route.publish({ route: 'input', inputAvailable: true, reason: '', labels });
  await nextTick();
  expect(input?.getAttribute('aria-selected')).toBe('true');
  output?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(route.choose).toHaveBeenLastCalledWith('output');
  mounted.dispose();
  route.publish({ route: 'output', inputAvailable: true, reason: '', labels });
});

test('reports the route tabs as the chrome that shared canvas surfaces keep clear', () => {
  const absent = mount(null);
  expect(absent.mounted.routeBounds()).toEqual([]);
  absent.mounted.dispose();
  const route = routePort({ route: 'input', inputAvailable: true, reason: '', labels });
  const { root, mounted } = mount(route.port);
  const tabs = root.querySelector('.sc-output-route')!;
  const bounds: DOMRect = {
    x: 108,
    y: 58,
    left: 108,
    top: 58,
    width: 140,
    height: 40,
    right: 248,
    bottom: 98,
    toJSON: () => ({}),
  };
  jest.spyOn(tabs, 'getBoundingClientRect').mockReturnValue(bounds);
  expect(mounted.routeBounds()).toEqual([bounds]);
  mounted.dispose();
});
