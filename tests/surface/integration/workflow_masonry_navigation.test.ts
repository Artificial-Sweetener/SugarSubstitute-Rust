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
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { WorkflowMasonryHostLifecycle } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryHostLifecycle.js';
import { WorkflowMasonryTransition } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryTransition.js';
import type { NativeNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('closes an open Properties panel before masonry mounts and blocks every native setting write', async () => {
  const setup = fixture();
  const mount = setup.mount.getMockImplementation();
  if (!mount) throw new Error('Native renderer fixture missing.');
  setup.mount.mockImplementation((...args) => {
    expect(setup.propertiesOpen()).toBe(false);
    return mount(...args);
  });
  for (let visit = 0; visit < 3; visit += 1) {
    await setup.setPropertiesOpen(true);
    setup.host.toggle();
    expect(setup.propertiesOpen()).toBe(false);
    await settle();
    await setup.setPropertiesOpen(true);
    expect(setup.propertiesOpen()).toBe(false);
    setup.setMode('app');
    expect(setup.propertiesOpen()).toBe(true);
    await setup.setPropertiesOpen(true);
    expect(setup.propertiesOpen()).toBe(true);
    setup.setMode('graph');
    expect(setup.propertiesOpen()).toBe(false);
    await settle();
    setup.host.toggle();
    await settle();
    expect(setup.propertiesOpen()).toBe(true);
  }
  setup.host.toggle();
  await settle();
  setup.setNodes2(false);
  await settle();
  expect(setup.propertiesOpen()).toBe(true);
  setup.host.dispose();
  expect(setup.panelDefinition.migrateDeprecatedValue).toBeUndefined();
  setup.changes.dispose();
});

test.each([false, true])(
  'restores saved Properties state %s only after exit travel',
  async (open) => {
    const setup = fixture();
    await setup.setPropertiesOpen(open);
    let completeExit: (() => void) | undefined;
    jest
      .spyOn(WorkflowMasonryTransition.prototype, 'run')
      .mockImplementation(async (_view, entering) => {
        if (!entering)
          await new Promise<void>((resolve) => {
            completeExit = resolve;
          });
      });
    setup.host.toggle();
    await settle();
    setup.host.toggle();
    await settle();
    expect(completeExit).toBeDefined();
    expect(document.querySelector('.sugarcubes-workflow-masonry')).not.toBeNull();
    await setup.setPropertiesOpen(true);
    expect(setup.propertiesOpen()).toBe(false);
    completeExit?.();
    await settle();
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
    expect(setup.propertiesOpen()).toBe(open);
    await setup.setPropertiesOpen(true);
    expect(setup.propertiesOpen()).toBe(true);
    setup.host.dispose();
    setup.changes.dispose();
  },
);

test('keeps graph view when the native Properties close operation cannot commit', async () => {
  const setup = fixture();
  await setup.setPropertiesOpen(true);
  setup.rejectPropertiesClose();
  setup.host.toggle();
  await settle();
  expect(setup.propertiesOpen()).toBe(true);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(setup.mount).not.toHaveBeenCalled();
  expect(setup.reportError).toHaveBeenCalled();
  setup.host.dispose();
  setup.changes.dispose();
});

test('borrows the application renderer and releases only masonry mounts', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  expect(setup.getRenderer).toHaveBeenCalledTimes(1);
  setup.host.toggle();
  await settle();
  setup.host.toggle();
  await settle();
  expect(setup.getRenderer).toHaveBeenCalledTimes(2);
  setup.host.dispose();
  expect(setup.unmount).toHaveBeenCalledTimes(2);
  expect(setup.renderer.dispose).not.toHaveBeenCalled();
  setup.changes.dispose();
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

test('releases observers during Nodes 1 fallback and restores the view when Nodes 2 returns', async () => {
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

test('remembers graph and masonry across repeated app mode round trips', async () => {
  const setup = fixture();
  for (const mode of ['app', 'builder:arrange']) {
    setup.setMode(mode);
    setup.host.toggle();
    await settle();
    expect(setup.mount).not.toHaveBeenCalled();
    setup.setMode('graph');
    await settle();
    expect(setup.mount).not.toHaveBeenCalled();
  }
  setup.host.toggle();
  await settle();
  for (const mode of ['app', 'builder:arrange', 'app']) {
    setup.setMode(mode);
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
    expect(setup.root.add).toBe(setup.rootAdd);
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    setup.setMode('graph');
    await settle();
    expect(document.querySelector('input')?.value).toBe('root model');
    expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  }
  setup.host.toggle();
  await settle();
  setup.setMode('app');
  setup.setMode('graph');
  await settle();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  setup.host.dispose();
  expect(setup.modeListeners.size).toBe(0);
  setup.changes.dispose();
});

test('keeps app suspension through configuration and restores only the latest graph', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  setup.setMode('app');
  setup.host.beforeConfigureGraph();
  setup.canvas.setGraph(setup.nested);
  setup.host.afterConfigureGraph();
  await settle();
  expect(document.querySelector('input')).toBeNull();
  setup.setMode('graph');
  setup.setMode('app');
  await settle();
  expect(document.querySelector('input')).toBeNull();
  setup.setMode('builder:inputs');
  await settle();
  expect(document.querySelector('input')?.value).toBe('cube model');
  setup.setMode('builder:outputs');
  await settle();
  expect(setup.getRenderer).toHaveBeenCalledTimes(2);
  setup.host.dispose();
  setup.setMode('graph');
  await settle();
  expect(document.querySelector('input')).toBeNull();
  setup.changes.dispose();
});

test('preserves the requested view through Nodes 1 fallback during app mode', async () => {
  const setup = fixture();
  setup.host.toggle();
  await settle();
  setup.setMode('app');
  setup.setNodes2(false);
  await settle();
  setup.setMode('graph');
  await settle();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(setup.notifyRendererFallback).toHaveBeenCalledTimes(1);
  setup.setNodes2(true);
  await settle();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).not.toBeNull();
  setup.host.dispose();
  setup.changes.dispose();
});

test('cancels a partially mounted surface when app mode interrupts entry', async () => {
  const setup = fixture(20);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  setup.host.toggle();
  await settle();
  expect(setup.mount).toHaveBeenCalledTimes(6);
  setup.setMode('app');
  expect(setup.unmount).toHaveBeenCalledTimes(6);
  while (frames.length) {
    frames.shift()?.(0);
    await settle();
  }
  expect(setup.mount).toHaveBeenCalledTimes(6);
  expect(document.querySelector('input')).toBeNull();
  setup.setMode('graph');
  while (frames.length) {
    frames.shift()?.(0);
    await settle();
  }
  expect(document.querySelectorAll('input')).toHaveLength(20);
  expect(setup.reportError).not.toHaveBeenCalled();
  setup.host.dispose();
  setup.changes.dispose();
});

/** Mount real masonry with only the external native renderer replaced. */
function fixture(rootCount = 1) {
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    queueMicrotask(() => callback(0));
    return 1;
  });
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
  const renderer: NativeNodeCardRenderer = {
    mount,
    dispose: jest.fn(),
  };
  const getRenderer = jest.fn(async () => renderer);
  const cardState = new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} });
  const reportError = jest.fn();
  const notifyRendererFallback = jest.fn<() => void>();
  const modeListeners = new Set<() => void>();
  const workflow = {
    activeWorkflow: { activeMode: 'graph', initialMode: 'graph' },
    $subscribe(listener: () => void) {
      modeListeners.add(listener);
      return () => modeListeners.delete(listener);
    },
  };
  let propertiesOpen = false;
  let rejectPropertiesClose = false;
  const panelDefinition: { migrateDeprecatedValue?: (value: unknown) => unknown } = {};
  const setting = {
    settings: { 'Comfy.RightSidePanel.IsOpen': panelDefinition },
    get: () => propertiesOpen,
    async set(_id: string, value: unknown) {
      if (rejectPropertiesClose && value === false) throw new Error('Native close failed.');
      const normalized = panelDefinition.migrateDeprecatedValue?.(value) ?? value;
      if (typeof normalized !== 'boolean') throw new Error('Invalid panel setting.');
      propertiesOpen = normalized;
    },
  };
  let viewChoice = { canvas: true, masonry: false };
  const host = new WorkflowMasonryHostLifecycle({
    presentation: {
      ready: true,
      get snapshot() {
        return viewChoice;
      },
      subscribe: () => () => undefined,
      change(value) {
        viewChoice = { ...viewChoice, ...value };
      },
    },
    stack: {
      appearance: {
        appearance: { fluent: false, dark: true },
        subscribeAppearance: () => () => undefined,
      },
      getReorder: () => null,
      resolveSource: () => null,
      openCart() {},
      remove() {},
      contextMenu() {},
      closeTransient() {},
    },
    app: {
      extensionManager: { workflow, setting },
      graph: root,
      canvas,
      ui: {
        settings: {
          getSettingValue: (id: string) => (id === 'Comfy.VueNodes.Enabled' ? nodes2 : undefined),
          addSetting() {},
          setSettingValue() {},
          addEventListener: settingsEvents.addEventListener.bind(settingsEvents),
          removeEventListener: settingsEvents.removeEventListener.bind(settingsEvents),
        },
      },
    },
    document,
    getContainer: () => container,
    getCubes: () => [],
    getCardState: () => cardState,
    getGraphChanges: () => changes,
    getConnections: () => [],
    getRenderer,
    notifyRendererFallback,
    reportError,
  });
  host.setup();
  return {
    host,
    panelDefinition,
    propertiesOpen: () => propertiesOpen,
    setPropertiesOpen: (value: boolean) => setting.set('Comfy.RightSidePanel.IsOpen', value),
    rejectPropertiesClose: () => {
      rejectPropertiesClose = true;
    },
    modeListeners,
    setMode(mode: string) {
      workflow.activeWorkflow.activeMode = mode;
      for (const listener of modeListeners) listener();
    },
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
    renderer,
    getRenderer,
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
