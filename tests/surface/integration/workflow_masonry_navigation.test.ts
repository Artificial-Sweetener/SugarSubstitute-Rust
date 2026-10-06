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
/** Prove masonry follows native graph navigation and releases obsolete bindings. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ComfyCanvasGraphChangeAdapter } from '../../../frontend/comfyui/ui/surface/ComfyCanvasGraphChangeAdapter.js';
import { CubeSurfaceRendererLifecycle } from '../../../frontend/comfyui/ui/surface/CubeSurfaceRendererLifecycle.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { WorkflowMasonryHostLifecycle } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryHostLifecycle.js';
import type { NativeNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('replaces native bindings on root/cube navigation even when node ids and labels match', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  const oldInput = document.querySelector('input');
  expect(oldInput?.value).toBe('root model');
  setup.canvas.setGraph(setup.nested);
  expect(setup.unmount).toHaveBeenCalledWith(setup.root._nodes?.[0]);
  await settle();
  const input = document.querySelector('input');
  expect(input).not.toBe(oldInput);
  expect(input?.value).toBe('cube model');
  expect(setup.root.add).toBe(setup.rootAdd);
  expect(setup.nested.add).not.toBe(setup.nestedAdd);
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  input!.value = 'chosen cube model';
  input!.dispatchEvent(new Event('input'));
  setup.canvas.setGraph(setup.root);
  await settle();
  expect(document.querySelector('input')?.value).toBe('root model');
  expect(setup.nested._nodes?.[0]?.widgets?.[0]?.value).toBe('chosen cube model');
  expect(setup.root._nodes?.[0]?.widgets?.[0]?.value).toBe('root model');
  expect(setup.nested.add).toBe(setup.nestedAdd);
  setup.host.dispose();
  expect(setup.root.add).toBe(setup.rootAdd);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  const mounts = setup.mount.mock.calls.length;
  setup.canvas.setGraph(setup.nested);
  await settle();
  expect(setup.mount).toHaveBeenCalledTimes(mounts);
  expect(setup.reportError).not.toHaveBeenCalled();
  setup.changes.dispose();
});

test('cancels obsolete native mount batches when navigation interrupts mounting', async () => {
  const setup = fixture(20);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  setup.host.toggle();
  await settle();
  expect(setup.mount).toHaveBeenCalledTimes(6);
  setup.canvas.setGraph(setup.nested);
  expect(setup.unmount).toHaveBeenCalledTimes(6);
  await settle();
  while (frames.length) {
    frames.shift()?.(0);
    await settle();
  }
  expect(
    setup.mount.mock.calls.filter(([, node]) => setup.root._nodes?.includes(node)),
  ).toHaveLength(6);
  expect(document.querySelector('input')?.value).toBe('cube model');
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  setup.host.dispose();
  setup.changes.dispose();
});

test('binds the new runtime navigation source after workflow configuration', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  setup.host.beforeConfigureGraph();
  expect(setup.root.add).toBe(setup.rootAdd);
  setup.replaceChanges();
  setup.canvas.setGraph(setup.nested);
  setup.host.afterConfigureGraph();
  await settle();
  expect(document.querySelector('input')?.value).toBe('cube model');
  setup.canvas.setGraph(setup.root);
  await settle();
  expect(document.querySelector('input')?.value).toBe('root model');
  setup.host.dispose();
  setup.changes.dispose();
});

test('releases navigation and topology observers during Nodes 1 fallback', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  setup.setNodes2(false);
  await settle();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(setup.notifyRendererFallback).toHaveBeenCalledTimes(1);
  expect(setup.root.add).toBe(setup.rootAdd);
  const mounts = setup.mount.mock.calls.length;
  setup.canvas.setGraph(setup.nested);
  await settle();
  expect(setup.mount).toHaveBeenCalledTimes(mounts);
  setup.setNodes2(true);
  await settle();
  setup.host.toggle();
  await settle();
  expect(document.querySelector('input')?.value).toBe('cube model');
  setup.host.dispose();
  setup.changes.dispose();
});

test('restores graph presentation if the new native graph cannot mount', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  const failure = new Error('native new graph mount failed');
  setup.mount.mockImplementation(() => {
    throw failure;
  });
  setup.canvas.setGraph(setup.nested);
  await settle();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
  expect(setup.root.add).toBe(setup.rootAdd);
  expect(setup.nested.add).toBe(setup.nestedAdd);
  expect(setup.reportError).toHaveBeenCalledWith(failure);
  expect(setup.nested._nodes?.[0]?.widgets?.[0]?.value).toBe('cube model');
  setup.host.dispose();
  setup.changes.dispose();
});

/** Mount real masonry with only the external native renderer replaced. */
function fixture(rootCount = 1) {
  const root = graph('root model', rootCount);
  const nested = graph('cube model');
  const rootAdd = root.add;
  const nestedAdd = nested.add;
  const canvas = {
    graph: root,
    setGraph(graph: ComfyGraph): void {
      this.graph = graph;
    },
  };
  let changes = new ComfyCanvasGraphChangeAdapter(canvas);
  const settingsEvents = new EventTarget();
  let nodes2 = true;
  const container = document.createElement('div');
  container.className = 'graph-canvas-panel';
  document.body.append(container);
  const unmount = jest.fn<(node: ComfyNode) => void>();
  const mount = jest.fn<NativeNodeCardRenderer['mount']>((target, node) => {
    const input = document.createElement('input');
    input.value = String(node.widgets?.[0]?.value);
    input.addEventListener('input', () => {
      if (node.widgets?.[0]) node.widgets[0].value = input.value;
    });
    target.append(input);
    return { refresh() {}, unmount: () => unmount(node) };
  });
  jest.spyOn(CubeSurfaceRendererLifecycle.prototype, 'getRenderer').mockResolvedValue({
    mount,
    dispose() {},
  });
  const cardState = new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} });
  const reportError = jest.fn();
  const notifyRendererFallback = jest.fn<() => void>();
  const host = new WorkflowMasonryHostLifecycle({
    app: {
      graph: root,
      canvas,
      ui: {
        settings: {
          getSettingValue: () => nodes2,
          addEventListener: settingsEvents.addEventListener.bind(settingsEvents),
          removeEventListener: settingsEvents.removeEventListener.bind(settingsEvents),
        },
      },
    },
    document,
    getCubes: () => [],
    getCardState: () => cardState,
    getGraphChanges: () => changes,
    getConnections: () => [],
    notifyRendererFallback,
    reportError,
    logger: { warn: jest.fn() },
  });
  host.setup();
  return {
    host,
    get changes() {
      return changes;
    },
    replaceChanges() {
      changes.dispose();
      changes = new ComfyCanvasGraphChangeAdapter(canvas);
    },
    setNodes2(value: boolean) {
      nodes2 = value;
      settingsEvents.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    },
    canvas,
    root,
    nested,
    rootAdd,
    nestedAdd,
    container,
    mount,
    unmount,
    notifyRendererFallback,
    reportError,
  };
}

/** Give separate graphs deliberately identical presentation identifiers. */
function graph(value: string, count = 1): ComfyGraph {
  return {
    _nodes: Array.from({ length: count }, (_, id) => ({
      id: id + 1,
      type: 'ModelLoader',
      widgets: [{ name: 'model', value }],
    })),
    add() {},
    remove() {},
  };
}

/** Drain deterministic native mount and host notification continuations. */
async function settle(): Promise<void> {
  for (let index = 0; index < 20; index += 1) await Promise.resolve();
}
