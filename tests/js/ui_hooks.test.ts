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
import { describe, expect, test, beforeEach, jest } from '@jest/globals';
import { app } from './mocks/app.js';
import { api } from './mocks/api.js';
import type { MockSidebarTab } from './mocks/app.js';
import type { MockCanvas } from './mocks/app.js';
import type { ComfyGraph } from '../../web/comfyui/ui/types/graph.js';
import type {
  CubeContainmentService,
  ContainmentIndex,
} from '../../web/comfyui/ui/layout/CubeContainmentService.js';
import type { CubeCollisionService } from '../../web/comfyui/ui/layout/CubeCollisionService.js';

type EnforceForNodes = CubeContainmentService['enforceForNodes'];
type BuildContainmentIndex = CubeContainmentService['buildIndex'];
type ResolveCollisions = CubeCollisionService['resolveCollisions'];

interface HookGraph extends ComfyGraph {
  onNodeConnectionChange(...args: unknown[]): unknown;
}

interface HookCanvas extends MockCanvas {
  processMouseUp(...args: unknown[]): unknown;
  onNodeMoved(...args: unknown[]): unknown;
  setDirty: jest.Mock<(foreground?: boolean, background?: boolean) => void>;
}

function hookedGraph(): HookGraph {
  if (!app.graph) throw new Error('Expected hooked graph');
  return app.graph as HookGraph;
}

function hookedCanvas(): HookCanvas {
  return app.canvas as HookCanvas;
}

function containmentResult(instances: Set<string> = new Set<string>()) {
  return { clamped: 0, expanded: 0, instances };
}

function collisionResult(moved = false) {
  return { moved, iterations: 0 };
}

function emptyContainmentIndex(): ContainmentIndex {
  return {
    instances: [],
    instanceByNodeId: new Map(),
    instanceByMarkerId: new Map(),
  };
}

class TestLGraphCanvas {
  [key: string]: unknown;
}

class TestLGraphGroup {
  [key: string]: unknown;
  title: string;

  constructor(title = '') {
    this.title = title;
  }
}

class TestLGraphNode {
  [key: string]: unknown;
  flags: { collapsed?: boolean } = {};
  id?: number;
  graph?: unknown;
  pos?: number[];
  size?: number[];

  collapse(): boolean {
    this.flags.collapsed = this.flags.collapsed !== true;
    return this.flags.collapsed;
  }
}

async function loadUi() {
  const cacheBust = `?v=${Math.random().toString(36).slice(2)}`;
  return import(`../../web/comfyui/ui.js${cacheBust}`);
}

beforeEach(() => {
  app.reset();
  app.graph = {
    _nodes: [],
    _groups: [],
    onNodeAdded: () => {},
    onNodeRemoved: () => {},
    onNodeConnectionChange: () => {},
  };
  app.canvas = {
    graph: app.graph,
    setDirty: jest.fn(),
    onAfterChange: () => {},
    onDrawForeground: () => {},
    onDrawBackground: () => {},
    processMouseMove: () => {},
    processMouseDown: () => {},
    processMouseUp: () => {},
    onNodeMoved: () => {},
  };
  app.extensionManager = { registerSidebarTab: () => {} };
  app.clean = () => {};

  localStorage.clear();

  globalThis.requestAnimationFrame = (callback) => {
    callback(0);
    return 1;
  };
  globalThis.cancelAnimationFrame = () => {};
  globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;
  globalThis.window.cancelAnimationFrame = globalThis.cancelAnimationFrame;

  Object.assign(globalThis.window, {
    SugarCubes: {} as SugarCubesPublicApi,
    comfyAPI: { vueApp: { config: { globalProperties: { $toast: null } } } },
  });
  globalThis.LiteGraph = {
    LGraphCanvas: TestLGraphCanvas,
    LGraphNode: TestLGraphNode,
    LGraphGroup: TestLGraphGroup,
    LinkDirection: { LEFT: 3, RIGHT: 4 },
  } as unknown as LiteGraphHost;
  delete TestLGraphCanvas.prototype.__sugarcubes_proximity_hooked;
  delete TestLGraphCanvas.prototype.__sugarcubes_overlay_hooked;
  delete TestLGraphNode.prototype.__sugarcubes_collapse_hooked;
  delete TestLGraphNode.prototype.__sugarcubes_collapse_manager;
  TestLGraphCanvas.prototype.drawConnections = () => {};
  TestLGraphCanvas.prototype.drawForeground = () => {};
  TestLGraphNode.prototype.collapse = function collapse() {
    this.flags = this.flags || {};
    this.flags.collapsed = this.flags.collapsed !== true;
    return this.flags.collapsed;
  };

  api.fetchApi = async () => ({
    ok: true,
    json: async () => ({ cubes: [] }),
  });
  globalThis.fetch = async () => new Response('sdxl\nsd\n', { status: 200 });
});

describe('ui hooks and scheduling', () => {
  test('registerSidebarTab is called only once', async () => {
    const calls: MockSidebarTab[] = [];
    app.extensionManager.registerSidebarTab = (payload) => calls.push(payload);

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();
    await extension.setup!();

    expect(calls).toHaveLength(1);
  });

  test('ensureGraphHooks wraps onAfterChange only once', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();
    const wrapped = app.canvas.onAfterChange;

    await extension.setup!();
    expect(app.canvas.onAfterChange).toBe(wrapped);
  });

  test('scheduleCubeInstanceRefresh coalesces raf', async () => {
    const rafCalls: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCalls.push(callback);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    window.SugarCubes.scheduleCubeInstanceRefresh();
    window.SugarCubes.scheduleCubeInstanceRefresh();

    expect(rafCalls).toHaveLength(1);
  });

  test('scheduleCubeDirtyRefresh coalesces raf and accepts explicit graph', async () => {
    const rafCalls: FrameRequestCallback[] = [];
    const rafCallbacks: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCalls.push(callback);
      rafCallbacks.push(callback);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    app.graph = null;
    const graph = { _nodes: [], _groups: [] };
    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const refreshSpy = jest.spyOn(ui.dirtyManager, 'refresh');

    window.SugarCubes.scheduleCubeDirtyRefresh({ graph });
    window.SugarCubes.scheduleCubeDirtyRefresh({ graph });

    rafCallbacks.forEach((callback) => callback?.(0));
    expect(refreshSpy).toHaveBeenCalledWith(expect.objectContaining({ graph }));
  });

  test('placement payload shift keeps group bounds relative to shifted origin', async () => {
    await loadUi();
    const { buildShiftedPlacementPayload } = await loadUi();

    const payload = {
      nodes: [{ symbol: 'node_main', layout: { pos: [120, 230], size: [220, 140] } }],
      markers: [
        {
          alias: 'marker_in',
          layout: { id: 41, pos: [200, 240], size: [180, 64] },
          widget_values: { instance_id: 'legacy-inst' },
        },
      ],
      layout: {
        origin: [100, 200],
        groups: [
          {
            title: 'Shifted Group',
            bounding: [10, 20, 300, 400],
            sugarcubes: {
              managed: true,
              instance_id: 'legacy-inst',
              markers: { inputs: [41], outputs: [] },
              bounds: {
                x: 110,
                y: 220,
                w: 300,
                h: 400,
              },
            },
          },
        ],
      },
    };

    const shifted = buildShiftedPlacementPayload(payload, [50, -30], [150, 170]) as
      | typeof payload
      | null;
    if (!shifted) throw new Error('Expected shifted placement payload');

    expect(shifted.layout.origin).toEqual([150, 170]);
    expect(shifted.nodes[0].layout.pos).toEqual([170, 200]);
    expect(shifted.markers[0].layout.pos).toEqual([250, 210]);
    expect(shifted.layout.groups[0].bounding).toEqual([10, 20, 300, 400]);
    expect(shifted.layout.groups[0].sugarcubes.instance_id).toEqual(expect.any(String));
    expect(shifted.layout.groups[0].sugarcubes.instance_id).not.toBe('legacy-inst');
    expect(shifted.markers[0].widget_values.instance_id).toBe(
      shifted.layout.groups[0].sugarcubes.instance_id,
    );
    expect(shifted.layout.groups[0].sugarcubes.bounds).toEqual({
      x: 160,
      y: 190,
      w: 300,
      h: 400,
    });

    expect(payload.layout.origin).toEqual([100, 200]);
    expect(payload.layout.groups[0].bounding).toEqual([10, 20, 300, 400]);
    expect(payload.layout.groups[0].sugarcubes.bounds).toEqual({
      x: 110,
      y: 220,
      w: 300,
      h: 400,
    });
    expect(payload.layout.groups[0].sugarcubes.instance_id).toBe('legacy-inst');
    expect(payload.markers[0].widget_values.instance_id).toBe('legacy-inst');
  });

  test('overlay hooks wrap drawConnections and drawForeground', async () => {
    const proto = TestLGraphCanvas.prototype;
    const originalConnections = proto.drawConnections;
    const originalForeground = proto.drawForeground;

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    expect(proto.__sugarcubes_proximity_hooked).toBe(true);
    expect(proto.__sugarcubes_overlay_hooked).toBe(true);
    expect(proto.drawConnections).not.toBe(originalConnections);
    expect(proto.drawForeground).not.toBe(originalForeground);
  });

  test('connection hook triggers proximity preview only for cube markers', async () => {
    const setDirty = hookedCanvas().setDirty;
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    setDirty.mockClear();

    hookedGraph().onNodeConnectionChange({ type: 'KSampler' });
    expect(setDirty).not.toHaveBeenCalled();

    hookedGraph().onNodeConnectionChange({ type: 'SugarCubes.CubeInput' });
    expect(setDirty).toHaveBeenCalled();
  });

  test('node moved triggers containment before collision and dedupes per raf', async () => {
    const rafCalls: FrameRequestCallback[] = [];
    const rafCallbacks: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCalls.push(callback);
      rafCallbacks.push(callback);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const enforce = jest.fn<EnforceForNodes>(() => containmentResult(new Set(['inst-1'])));
    const collide = jest.fn<ResolveCollisions>(() => collisionResult(true));
    ui.containmentService.enforceForNodes = enforce;
    ui.collisionService.resolveCollisions = collide;

    hookedCanvas().onNodeMoved({ id: 1, pos: [0, 0], size: [10, 10] });
    hookedCanvas().onNodeMoved({ id: 1, pos: [0, 0], size: [10, 10] });

    rafCallbacks.forEach((callback) => callback?.(0));

    expect(enforce).toHaveBeenCalledTimes(1);
    expect(collide).toHaveBeenCalledTimes(1);
    expect(enforce.mock.invocationCallOrder[0]).toBeLessThan(collide.mock.invocationCallOrder[0]);
  });

  test('node moved uses selectedItems when callback node is wrong', async () => {
    const rafCallbacks: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCallbacks.push(callback);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const enforce = jest.fn<EnforceForNodes>(() => containmentResult());
    ui.containmentService.enforceForNodes = enforce;
    ui.collisionService.resolveCollisions = jest.fn<ResolveCollisions>(() => collisionResult());

    const movedNode = { id: 42, pos: [10, 20], size: [30, 40], type: 'KSampler' };
    const wrongNode = { id: 999, pos: [0, 0], size: [10, 10], type: 'KSampler' };
    app.canvas.selectedItems = new Set([movedNode]);
    hookedCanvas().onNodeMoved(wrongNode);

    rafCallbacks.forEach((callback) => callback?.(0));

    expect(enforce).toHaveBeenCalledTimes(1);
    expect(enforce.mock.calls[0]?.[0]?.nodes).toEqual([movedNode]);
  });

  test('onAfterChange enforces containment for selected nodes', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const enforce = jest.fn<EnforceForNodes>(() => containmentResult());
    ui.containmentService.enforceForNodes = enforce;
    ui.collisionService.resolveCollisions = jest.fn<ResolveCollisions>(() => collisionResult());

    const movedNode = { id: 77, pos: [5, 6], size: [10, 12], type: 'KSampler' };
    app.canvas.selectedItems = new Set([movedNode]);
    app.canvas.onAfterChange();

    expect(enforce).toHaveBeenCalledTimes(1);
    expect(enforce.mock.calls[0]?.[0]?.nodes).toEqual([movedNode]);
  });

  test('expanding a node clamps it after bounds settle', async () => {
    const rafCallbacks: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCallbacks.push(callback);
      return rafCallbacks.length;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const enforce = jest.fn<EnforceForNodes>(() => containmentResult());
    ui.containmentService.enforceForNodes = enforce;
    ui.collisionService.resolveCollisions = jest.fn<ResolveCollisions>(() => collisionResult());

    const node = new TestLGraphNode();
    node.id = 314;
    node.graph = app.graph;
    node.flags = { collapsed: true };
    node.pos = [10, 12];
    node.size = [100, 120];
    node.collapse();

    expect(node.flags.collapsed).toBe(false);
    expect(enforce).not.toHaveBeenCalled();

    let safety = 0;
    while (rafCallbacks.length > 0 && safety < 20) {
      rafCallbacks.shift()?.(0);
      safety += 1;
    }

    expect(enforce).toHaveBeenCalled();
    expect(
      enforce.mock.calls.some(
        (args) => Array.isArray(args?.[0]?.nodes) && args[0].nodes[0] === node,
      ),
    ).toBe(true);
  });

  test('group drag commits moved group bounds to metadata', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const indexStub = emptyContainmentIndex();
    ui.containmentService.buildIndex = jest.fn<BuildContainmentIndex>(() => indexStub);
    const collide = jest.fn<ResolveCollisions>(() => collisionResult());
    ui.collisionService.resolveCollisions = collide;

    const metadata = {
      managed: true,
      instance_id: 'inst-group-1',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];

    app.canvas.selected_group = group;
    app.canvas.processMouseDown({});
    group.pos[0] = 140;
    group.pos[1] = 260;
    hookedCanvas().processMouseUp({});

    expect(metadata.bounds.x).toBe(140);
    expect(metadata.bounds.y).toBe(260);
    expect(group.pos).toEqual([140, 260]);
    expect(collide).toHaveBeenCalledWith(
      expect.objectContaining({
        graph: app.graph,
        activeInstanceId: 'inst-group-1',
        index: indexStub,
      }),
    );
  });

  test('group drag commit survives release-time group snapback', async () => {
    const metadata = {
      managed: true,
      instance_id: 'inst-group-2',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];

    hookedCanvas().processMouseUp = () => {
      group.pos[0] = 100;
      group.pos[1] = 200;
    };

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    app.canvas.selected_group = group;
    app.canvas.processMouseDown({});
    group.pos[0] = 145;
    group.pos[1] = 255;
    hookedCanvas().processMouseUp({});

    expect(metadata.bounds.x).toBe(145);
    expect(metadata.bounds.y).toBe(255);
    expect(group.pos).toEqual([145, 255]);
  });

  test('group drag release fallback commits selected group when capture was missed', async () => {
    const metadata = {
      managed: true,
      instance_id: 'inst-group-3',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];

    hookedCanvas().processMouseUp = () => {
      group.pos[0] = 100;
      group.pos[1] = 200;
    };

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    app.canvas.selected_group = group;
    group.pos[0] = 155;
    group.pos[1] = 265;
    hookedCanvas().processMouseUp({});

    expect(metadata.bounds.x).toBe(155);
    expect(metadata.bounds.y).toBe(265);
    expect(group.pos).toEqual([155, 265]);
  });

  test('group resize commits bounds when canvas tracks resizingGroup', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const metadata = {
      managed: true,
      instance_id: 'inst-resize-group',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];

    app.canvas.selected_group = null;
    app.canvas.resizingGroup = group;
    app.canvas.selected_group_resizing = true;
    app.canvas.processMouseDown({});
    group.size[0] = 360;
    group.size[1] = 470;
    hookedCanvas().processMouseUp({});

    expect(metadata.bounds.w).toBe(360);
    expect(metadata.bounds.h).toBe(470);
    expect(group.size).toEqual([360, 470]);
  });

  test('onAfterChange commits pending drag bounds when group references clear', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const metadata = {
      managed: true,
      instance_id: 'inst-after-change-fallback',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];

    app.canvas.selected_group = group;
    app.canvas.selectedItems = new Set();
    app.canvas.processMouseDown({});
    app.canvas.selected_group = null;
    app.canvas.resizingGroup = null;
    app.canvas.selected_group_resizing = false;
    group.size[0] = 355;
    group.size[1] = 465;

    app.canvas.onAfterChange();

    expect(metadata.bounds.w).toBe(355);
    expect(metadata.bounds.h).toBe(465);
    expect(group.size).toEqual([355, 465]);
  });

  test('onAfterChange commits managed group bounds even without selected group state', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const indexStub = emptyContainmentIndex();
    ui.containmentService.buildIndex = jest.fn<BuildContainmentIndex>(() => indexStub);
    ui.containmentService.enforceForNodes = jest.fn<EnforceForNodes>(() => containmentResult());
    const collide = jest.fn<ResolveCollisions>(() => collisionResult());
    ui.collisionService.resolveCollisions = collide;

    const metadata = {
      managed: true,
      instance_id: 'inst-after-change-global',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [100, 200],
      size: [370, 480],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];
    app.canvas.selected_group = null;
    app.canvas.resizingGroup = null;
    app.canvas.selected_group_resizing = false;
    app.canvas.selectedItems = new Set();

    app.canvas.onAfterChange();

    expect(metadata.bounds.w).toBe(370);
    expect(metadata.bounds.h).toBe(480);
    expect(collide).toHaveBeenCalledWith(
      expect.objectContaining({
        graph: app.graph,
        activeInstanceId: 'inst-after-change-global',
        index: indexStub,
      }),
    );
  });

  test('onAfterChange commits selected managed group bounds before reconcile', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const ui = (await import('../../web/comfyui/ui/index.js')).getSugarCubesUI();
    const indexStub = emptyContainmentIndex();
    ui.containmentService.buildIndex = jest.fn<BuildContainmentIndex>(() => indexStub);
    ui.containmentService.enforceForNodes = jest.fn<EnforceForNodes>(() => containmentResult());
    const collide = jest.fn<ResolveCollisions>(() => collisionResult());
    ui.collisionService.resolveCollisions = collide;

    const metadata = {
      managed: true,
      instance_id: 'inst-after-change',
      bounds: {
        x: 100,
        y: 200,
        w: 300,
        h: 400,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: [140, 260],
      size: [300, 400],
      properties: { sugarcubes: metadata },
    };
    hookedGraph()._groups = [group];
    app.canvas.selected_group = group;
    app.canvas.selectedItems = new Set();

    app.canvas.onAfterChange();

    expect(metadata.bounds.x).toBe(140);
    expect(metadata.bounds.y).toBe(260);
    expect(collide).toHaveBeenCalledWith(
      expect.objectContaining({
        graph: app.graph,
        activeInstanceId: 'inst-after-change',
        index: indexStub,
      }),
    );
  });
});
