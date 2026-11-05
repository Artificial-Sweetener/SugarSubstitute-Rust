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
/** Prove the canvas selector inside the one canvas navigation bar. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import type {
  CanvasRouteId,
  CanvasRoutePort,
  CanvasRouteState,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasRoute.js';
import type { CanvasViewSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
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

/** Mount the overlay with a route port over a view snapshot. */
function mount(
  route: CanvasRoutePort | null,
  snapshot: CanvasViewSnapshot = canvasSnapshot('route'),
) {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(snapshot),
    jest.fn(),
    root,
    undefined,
    route,
  );
  return { root, mounted };
}

/** Read the canvas selector button. */
function canvasSelector(root: ParentNode): HTMLButtonElement | null {
  return root.querySelector<HTMLButtonElement>('.sc-output-route button[aria-haspopup="listbox"]');
}

/** Open the canvas selector and read its rows once the popup is rendered. */
async function openRows(root: ParentNode): Promise<HTMLButtonElement[]> {
  canvasSelector(root)?.click();
  await nextTick();
  await nextTick();
  return [...document.querySelectorAll<HTMLButtonElement>('[role="option"]')];
}

test('canvases without workflow surfaces show no canvas selector', () => {
  const { root, mounted } = mount(null);
  expect(root.querySelector('.sc-output-route')).toBeNull();
  mounted.dispose();
  const empty = routePort(null);
  const second = mount(empty.port);
  expect(second.root.querySelector('.sc-output-route')).toBeNull();
  second.mounted.dispose();
});

test('a workflow without input canvas nodes has no canvas selector at all', async () => {
  const route = routePort({ route: 'output', inputAvailable: false, labels });
  const { root, mounted } = mount(route.port);
  expect(root.querySelector('.sc-output-route')).toBeNull();
  expect(root.textContent).not.toContain('Input');
  route.publish({ route: 'output', inputAvailable: true, labels });
  await nextTick();
  expect(canvasSelector(root)?.textContent).toBe('Output');
  mounted.dispose();
});

test('the selector shows the current canvas and opens a picker that chooses the other', async () => {
  const route = routePort({ route: 'output', inputAvailable: true, labels });
  const { root, mounted } = mount(route.port);
  expect(root.querySelectorAll('.sc-output-route [role="tab"]')).toHaveLength(0);
  expect(canvasSelector(root)?.getAttribute('aria-label')).toBe('Canvas route');
  const rows = await openRows(root);
  expect(rows.map((row) => row.textContent)).toEqual(['Input', 'Output']);
  rows[1]?.click();
  expect(route.choose).not.toHaveBeenCalled();
  (await openRows(root))[0]?.click();
  expect(route.choose).toHaveBeenCalledWith('input');
  route.publish({ route: 'input', inputAvailable: true, labels });
  await nextTick();
  expect(canvasSelector(root)?.textContent).toBe('Input');
  mounted.dispose();
});

test('the canvas selector leads the output navigation in one bar, and Input shows only it', async () => {
  const route = routePort({ route: 'output', inputAvailable: true, labels });
  const { root, mounted } = mount(route.port);
  const bars = root.querySelectorAll('.sc-output-tabstrip');
  expect(bars).toHaveLength(1);
  expect(bars[0]?.firstElementChild?.classList.contains('sc-output-route')).toBe(true);
  expect(
    [...(bars[0]?.querySelectorAll('[role="tab"]') ?? [])].map((tab) => tab.textContent),
  ).toEqual(['Saved image', 'Detail']);
  route.publish({ route: 'input', inputAvailable: true, labels });
  await nextTick();
  const inputBars = root.querySelectorAll('.sc-output-tabstrip');
  expect(inputBars).toHaveLength(1);
  expect(inputBars[0]?.querySelectorAll('button')).toHaveLength(1);
  expect(canvasSelector(root)?.textContent).toBe('Input');
  mounted.dispose();
});

test('comparison keeps the canvas selector in the first image bar', () => {
  const snapshot = canvasSnapshot('compare');
  const comparing: CanvasViewSnapshot = {
    ...snapshot,
    view: {
      ...snapshot.view,
      comparison: { secondary: null, orientation: 'vertical', split: 0.5 },
    },
  };
  const route = routePort({ route: 'output', inputAvailable: true, labels });
  const { root, mounted } = mount(route.port, comparing);
  expect(root.querySelector('.sc-output-compare-first .sc-output-route')).not.toBeNull();
  expect(root.querySelector('.sc-output-compare-second .sc-output-route')).toBeNull();
  mounted.dispose();
});
