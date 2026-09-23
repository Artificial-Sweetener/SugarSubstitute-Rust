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
import { app } from '../../frontend_core/support/mocks/app.js';
import { api } from '../../frontend_core/support/mocks/api.js';
import { ProximityOverlay } from '../../../frontend/comfyui/ui/overlays/ProximityOverlay.js';
import type { ProximityMatch } from '../../../frontend/comfyui/ui/overlays/ProximityOverlay.js';
import type { OverlayManagerOptions } from '../../../frontend/comfyui/ui/overlays/OverlayManager.js';
import type { CubeChromeOverlay as CubeChromeOverlayType } from '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js';
import type { MockCanvas } from '../../frontend_core/support/mocks/app.js';
import type {
  ComfyGraph,
  ComfyLink,
  ComfyNode,
  GraphId,
} from '../../../frontend/comfyui/ui/types/graph.js';
import type { Vec2 } from '../../../frontend/comfyui/ui/types/common.js';
import type { ComfyApplication } from '../../../frontend/comfyui/ui/types/graph.js';
import { createFallbackIconModel } from '../../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';

interface TestGraph extends ComfyGraph {
  _nodes: ComfyNode[];
  links: Record<string, ComfyLink>;
  getLink(id: GraphId): ComfyLink | null;
}

class TestLGraphCanvas implements MockCanvas {
  [key: string]: unknown;
  selectedItems = new Set<unknown>();
  getNodeMenuOptions(_node: unknown): unknown[] {
    return [];
  }
  getCanvasMenuOptions(): unknown[] {
    return [];
  }
  onAfterChange(): void {}
  onDrawForeground(): void {}
  processMouseMove(): void {}
  processMouseDown(): void {}
  drawConnections(_ctx: CanvasRenderingContext2D): void {}
  drawForeground(_ctx: CanvasRenderingContext2D): void {}
  graph?: ComfyGraph;
  connections_width = 3;
  default_link_color = '#7fc4ff';
  setDirty = jest.fn();
  renderLink = jest.fn((_ctx: CanvasRenderingContext2D) => undefined);
  getLinkColor = (): string => '#7fc4ff';
  __testCtx: CanvasRenderingContext2D = canvasContext({
    save() {},
    restore() {},
    setLineDash() {},
    lineDashOffset: 0,
    strokeStyle: '#fff',
  });
}

class TestLGraphGroup {
  [key: string]: unknown;
  static padding = 0;
  title: string;

  constructor(title = '') {
    this.title = title;
  }
}

function canvasContext<T extends object>(value: T): T & CanvasRenderingContext2D {
  return value as T & CanvasRenderingContext2D;
}

function testCanvas(): TestLGraphCanvas {
  return app.canvas as TestLGraphCanvas;
}

function chromeAdapter() {
  return { getApp: () => app as unknown as ComfyApplication };
}

function mouseEvent(clientX: number, clientY: number): MouseEvent {
  return new MouseEvent('mousemove', { clientX, clientY });
}

type ChromeCanvasFixture = Parameters<CubeChromeOverlayType['handleMouseDown']>[1];

function chromeCanvas<T extends object>(value: T): T & ChromeCanvasFixture {
  return value as T & ChromeCanvasFixture;
}

async function loadUi() {
  const cacheBust = `?v=${Math.random().toString(36).slice(2)}`;
  return import(`../../../web/comfyui/ui.js${cacheBust}`);
}

function createTestGraph(): TestGraph {
  const graph = {
    _nodes: [] as ComfyNode[],
    links: {} as Record<string, ComfyLink>,
    getLink: (id: GraphId) => graph.links[String(id)] || null,
  };
  return graph;
}

beforeEach(() => {
  app.reset();
  app.graph = createTestGraph();
  const ctx = {
    save: () => {},
    restore: () => {},
    setLineDash: () => {},
    lineDashOffset: 0,
    strokeStyle: '#fff',
  };

  app.extensionManager = { registerSidebarTab: () => {} };
  app.clean = () => {};

  globalThis.requestAnimationFrame = (callback) => {
    callback(0);
    return 1;
  };
  globalThis.cancelAnimationFrame = () => {};

  globalThis.LiteGraph = {
    LGraphCanvas: TestLGraphCanvas,
    LinkDirection: { LEFT: 3, RIGHT: 4 },
    LinkMarkerShape: { None: 0 },
  } as unknown as LiteGraphHost;
  TestLGraphCanvas.prototype.drawConnections = () => {};
  TestLGraphCanvas.prototype.drawForeground = () => {};

  app.canvas = new TestLGraphCanvas();
  testCanvas().graph = app.graph;
  testCanvas().connections_width = 3;
  testCanvas().default_link_color = '#7fc4ff';
  testCanvas().setDirty = jest.fn();
  testCanvas().renderLink = jest.fn((_ctx: CanvasRenderingContext2D) => {
    _ctx.strokeStyle = '#abc';
  });
  testCanvas().getLinkColor = () => '#7fc4ff';
  testCanvas().__testCtx = canvasContext(ctx);

  Object.assign(globalThis.window, {
    SugarCubes: {} as SugarCubesPublicApi,
    comfyAPI: { vueApp: { config: { globalProperties: { $toast: null } } } },
  });
  globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;
  globalThis.window.cancelAnimationFrame = globalThis.cancelAnimationFrame;

  api.fetchApi = async () => ({ ok: true, json: async () => ({ cubes: [] }) });
  globalThis.fetch = async () => new Response('', { status: 200 });
});

describe('ui overlay rendering', () => {
  test('drawConnections does not fabricate Cube links before a container runtime exists', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    testCanvas().drawConnections(testCanvas().__testCtx);

    expect(testCanvas().renderLink).not.toHaveBeenCalled();
  });

  test('paints proximity through the stable canvas background hook', async () => {
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const outputNode: ComfyNode = {
      id: 'output',
      graph: app.graph,
      outputs: [{ name: 'image', type: 'IMAGE' }],
    };
    const inputNode: ComfyNode = {
      id: 'input',
      graph: app.graph,
      inputs: [{ name: 'image', type: 'IMAGE' }],
    };
    const manager = new OverlayManager({
      adapter: {
        getApp: () => app as unknown as ComfyApplication,
        getLiteGraph: () => globalThis.LiteGraph,
        getConsole: () => console,
      },
    });
    manager.proximity.setEndpointSource({
      discover: () => ({
        outputs: [
          {
            key: 'output',
            endpointId: 'output',
            node: outputNode,
            slot: 0,
            cube: 'cube-output',
            instanceId: 'output',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [0, 20],
            slotName: 'image',
            originId: 'output:leaf',
            originSlot: 0,
          },
        ],
        inputs: [
          {
            key: 'input',
            endpointId: 'input',
            node: inputNode,
            slot: 0,
            cube: 'cube-input',
            instanceId: 'input',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [80, 20],
            slotName: 'image',
            promptTargets: [{ nodeId: 'input:leaf', inputSlot: 0, inputName: 'image' }],
          },
        ],
      }),
    });
    manager.proximity.runPreview({ graph: app.graph });
    manager.ensureOverlayHook();

    const drawBackground = testCanvas().onDrawBackground;
    expect(typeof drawBackground).toBe('function');
    Reflect.apply(drawBackground as (...args: unknown[]) => unknown, testCanvas(), [
      testCanvas().__testCtx,
    ]);

    expect(testCanvas().renderLink).toHaveBeenCalledTimes(1);
  });

  test('refreshes proximity continuously from current Nodes 1 canvas drag state', async () => {
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const canvasElement = document.createElement('canvas');
    testCanvas().canvas = canvasElement;
    const outputNode: ComfyNode = {
      id: 'output',
      graph: app.graph,
      outputs: [{ name: 'image', type: 'IMAGE' }],
    };
    const inputNode: ComfyNode = {
      id: 'input',
      graph: app.graph,
      inputs: [{ name: 'image', type: 'IMAGE' }],
    };
    const graph = app.graph as TestGraph;
    graph._nodes.push(outputNode, inputNode);
    let inputPosition: Vec2 = [80, 20];
    const manager = new OverlayManager({
      adapter: {
        getApp: () => app as unknown as ComfyApplication,
        getLiteGraph: () => globalThis.LiteGraph,
        getConsole: () => console,
      },
      scheduler: {
        raf: (callback) => {
          callback(0);
          return 1;
        },
        timeout: () => 1,
      },
    });
    manager.proximity.setEndpointSource({
      discover: () => ({
        outputs: [
          {
            key: 'output',
            endpointId: 'output',
            node: outputNode,
            slot: 0,
            cube: 'cube-output',
            instanceId: 'output',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [0, 20],
            slotName: 'image',
            originId: 'output:leaf',
            originSlot: 0,
          },
        ],
        inputs: [
          {
            key: 'input',
            endpointId: 'input',
            node: inputNode,
            slot: 0,
            cube: 'cube-input',
            instanceId: 'input',
            alias: 'image',
            type: 'IMAGE',
            slotPos: inputPosition,
            slotName: 'image',
            promptTargets: [{ nodeId: 'input:leaf', inputSlot: 0, inputName: 'image' }],
          },
        ],
      }),
    });
    manager.ensureGraphHooks();

    canvasElement.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));

    expect(manager.proximity.overlayMatches[0]?.inputPos).toEqual([80, 20]);

    inputPosition = [120, 40];
    inputNode.pos = [120, 40];
    testCanvas().state = { draggingItems: true };
    testCanvas().selectedItems = new Set([inputNode]);
    canvasElement.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));

    expect(manager.proximity.overlayMatches[0]?.inputPos).toEqual([120, 40]);

    inputPosition = [500, 500];
    inputNode.pos = [500, 500];
    canvasElement.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));

    expect(manager.proximity.overlayMatches).toEqual([]);

    testCanvas().state = { draggingItems: false };
    testCanvas().selectedItems = new Set();
    canvasElement.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));

    expect(manager.proximity.overlayMatches).toEqual([]);
  });

  test('does not rediscover proximity while the pointer only hovers the graph', async () => {
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const canvasElement = document.createElement('canvas');
    const canvasContainer = document.createElement('div');
    canvasContainer.className = 'graph-canvas-container';
    canvasContainer.append(canvasElement);
    document.body.append(canvasContainer);
    testCanvas().canvas = canvasElement;
    const discover = jest.fn(() => ({ outputs: [], inputs: [] }));
    const manager = new OverlayManager({
      adapter: {
        getApp: () => app as unknown as ComfyApplication,
        getLiteGraph: () => globalThis.LiteGraph,
        getConsole: () => console,
      },
      scheduler: {
        raf: (callback) => {
          callback(0);
          return 1;
        },
        timeout: () => 1,
      },
    });
    manager.proximity.setEndpointSource({ discover });
    manager.ensureGraphHooks();
    discover.mockClear();

    canvasElement.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));
    canvasElement.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));

    expect(discover).not.toHaveBeenCalled();
    canvasContainer.remove();
  });

  test('attaches proximity graph mutation tracking', async () => {
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const graph = app.graph as TestGraph & {
      onNodeAdded(node: unknown): void;
    };
    const manager = new OverlayManager({
      adapter: {
        getApp: () => app as unknown as ComfyApplication,
        getLiteGraph: () => globalThis.LiteGraph,
        getConsole: () => console,
      },
      scheduler: {
        raf: () => 1,
        timeout: () => 1,
      },
    });
    manager.ensureGraphHooks();
    const schedulePreview = jest
      .spyOn(manager.proximity, 'schedulePreview')
      .mockImplementation(() => undefined);
    graph.onNodeAdded?.({});

    expect(schedulePreview).toHaveBeenCalledTimes(1);
    expect(schedulePreview).toHaveBeenCalledWith({ graph });
  });

  test('tracks link removal after Comfy switches the active workflow graph', async () => {
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const frames: FrameRequestCallback[] = [];
    const manager = new OverlayManager({
      adapter: {
        getApp: () => app as unknown as ComfyApplication,
        getLiteGraph: () => globalThis.LiteGraph,
        getConsole: () => console,
      },
      scheduler: {
        raf: (callback) => {
          frames.push(callback);
          return frames.length;
        },
        timeout: () => 1,
      },
    });
    manager.ensureGraphHooks();
    manager.ensureOverlayHook();
    const switchedGraph = createTestGraph() as TestGraph & {
      removeLink(linkId: unknown): unknown;
    };
    const removeLink = jest.fn((linkId: unknown) => linkId);
    switchedGraph.removeLink = removeLink;
    testCanvas().graph = switchedGraph;
    const drawBackground = testCanvas().onDrawBackground;
    expect(typeof drawBackground).toBe('function');
    Reflect.apply(drawBackground as (...args: unknown[]) => unknown, testCanvas(), [
      testCanvas().__testCtx,
    ]);
    frames.length = 0;
    const schedulePreview = jest
      .spyOn(manager.proximity, 'schedulePreview')
      .mockImplementation(() => undefined);

    expect(switchedGraph.removeLink('link-1')).toBe('link-1');
    expect(removeLink).toHaveBeenCalledWith('link-1');
    expect(schedulePreview).not.toHaveBeenCalled();
    frames.splice(0).forEach((callback) => callback(0));
    frames.splice(0).forEach((callback) => callback(16));
    expect(schedulePreview).toHaveBeenCalledWith({ graph: switchedGraph });
  });

  test('chrome overlay does not create DOM roots', async () => {
    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    const chromeRoot = document.querySelector('.sugarcubes-chrome-root');
    expect(chromeRoot).toBeNull();
  });

  test('chrome overlay draws centered name and source badge text', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-1',
      cube_id: 'Artificial-Sweetener/Base-Cubes/canonical.cube',
      default_alias: 'Canonical',
      instance_alias: 'Alias Name',
      cube_version: '1.2.3',
      dirty: false,
    };

    const group = { title: 'Alias Name' };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 700, 60], metadata, group);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'Canonical version 1.2.3',
      expect.any(Number),
      expect.any(Number),
    );
    expect(ctx.fillText).toHaveBeenCalledWith(
      'from Base-Cubes by Artificial-Sweetener',
      expect.any(Number),
      expect.any(Number),
    );
    expect(ctx.fillText).not.toHaveBeenCalledWith(
      'by Artificial-Sweetener/Base-Cubes',
      expect.any(Number),
      expect.any(Number),
    );
    const nameCall = ctx.fillText.mock.calls.find(([text]) => text === 'Canonical version 1.2.3');
    const sourceCall = ctx.fillText.mock.calls.find(
      ([text]) => text === 'from Base-Cubes by Artificial-Sweetener',
    );
    if (!nameCall || !sourceCall) throw new Error('Missing expected chrome text calls');
    expect(nameCall[1]).toBeGreaterThan(210);
    expect(sourceCall[1]).toBe(nameCall[1]);
  });

  test('chrome overlay strips leading v from version badge text', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-v-prefix',
      cube_id: 'Artificial-Sweetener/Base-Cubes/canonical.cube',
      default_alias: 'Canonical',
      cube_version: 'v1.2.3',
      dirty: false,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 360, 60], metadata);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'Canonical version 1.2.3',
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('chrome overlay preserves centered definition badge when instance title matches', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      fillRect: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-matching-title',
      cube_id: 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
      default_alias: 'SDXL/Text to Image',
      instance_alias: 'SDXL/Text to Image',
      cube_version: '2.0.0',
      dirty: false,
    };
    const group = { title: 'SDXL/Text to Image', color: '#3f789e' };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 700, 60], metadata, group);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'SDXL/Text to Image version 2.0.0',
      expect.any(Number),
      expect.any(Number),
    );
    expect(ctx.fillText).toHaveBeenCalledWith(
      'from Base-Cubes by Artificial-Sweetener',
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('managed group draw places cube icon before current title without mutating title', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    class LGraphGroup extends TestLGraphGroup {
      draw(
        this: { font_size: number; title: string; _pos: number[] },
        _canvasInstance: unknown,
        ctx: CanvasRenderingContext2D,
      ): void {
        ctx.font = `${this.font_size || 24}px Inter`;
        ctx.textAlign = 'left';
        ctx.fillText(
          this.title,
          this._pos[0]! + LGraphGroup.padding,
          this._pos[1]! + this.font_size,
        );
      }
    }
    LGraphGroup.padding = 4;
    globalThis.LiteGraph.LGraphGroup = LGraphGroup;
    globalThis.LiteGraph.GROUP_FONT = 'Inter';
    const overlay = new CubeChromeOverlay({
      adapter: { getLiteGraph: () => globalThis.LiteGraph },
    });
    const ctx = {
      font: '',
      textAlign: '',
      textBaseline: '',
      fillStyle: '',
      globalAlpha: 1,
      save: jest.fn(),
      restore: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      fillText: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
    };
    const group = {
      title: 'Text to Image',
      _pos: [10, 20],
      _size: [420, 120],
      _bounding: [10, 20, 420, 120],
      font_size: 24,
      color: '#3f789e',
      properties: {
        sugarcubes: {
          managed: true,
          instance_id: 'instance-title-icon',
          cube_id: 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
          default_alias: 'SDXL/Text to Image',
          instance_alias: 'Text to Image',
        },
      },
    };

    overlay.setup();
    LGraphGroup.prototype.draw.call(group, { editor_alpha: 1 }, canvasContext(ctx));

    const nativeBlankCall = ctx.fillText.mock.calls.find(([text]) => text === '');
    const iconCall = ctx.fillText.mock.calls.find(([text]) => text === 'TI');
    const titleCall = ctx.fillText.mock.calls.find(([text]) => text === 'Text to Image');
    expect(nativeBlankCall).toBeTruthy();
    if (!iconCall || !titleCall) throw new Error('Missing managed group title calls');
    expect(Number(iconCall[1])).toBeLessThan(Number(titleCall[1]));
    expect(Number(titleCall[1])).toBeGreaterThan(40);
    expect(group.title).toBe('Text to Image');
  });

  test('chrome definition initials use normalized fallback font without decorative box chrome', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const fontCalls: string[] = [];
    const ctx = {
      font: '',
      fillStyle: '',
      textAlign: '',
      textBaseline: '',
      save: jest.fn(),
      restore: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      measureText: jest.fn(() => ({ width: 12 })),
      fillText: jest.fn(function recordFallbackFont(this: { font: string }) {
        fontCalls.push(this.font);
      }),
    };

    overlay.drawDefinitionIcon(canvasContext(ctx), createFallbackIconModel('TI'), 10, 12, 24);

    expect(ctx.fillText).toHaveBeenCalledWith('TI', 0, 0);
    expect(ctx.translate).toHaveBeenCalledWith(10, 12);
    expect(ctx.scale).toHaveBeenCalledWith(0.25, 0.25);
    expect(fontCalls.at(-1)).toContain('700');
    expect(fontCalls.at(-1)).toContain('62px');
    expect(fontCalls.at(-1)).toContain('Segoe UI');
    expect(ctx.beginPath).not.toHaveBeenCalled();
    expect(ctx.fill).not.toHaveBeenCalled();
    expect(ctx.stroke).not.toHaveBeenCalled();
  });

  test('chrome overlay hit regions exclude badge and badge hover is non-interactive', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const actions = { onSave: jest.fn(), onClone: jest.fn() };
    const overlay = new CubeChromeOverlay({ adapter, actions });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-2',
      default_alias: 'Cube Hover',
      instance_alias: 'Hover Alias',
      dirty: true,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 360, 60], metadata);

    expect(overlay.getDebugState().hitRegions.length).toBeGreaterThan(0);
    expect(overlay.getDebugState().hitRegions.some((region) => region.key === 'menu')).toBe(true);
    expect(
      overlay.getDebugState().hitRegions.find((region) => region.key === 'menu')?.tooltip,
    ).toBe('Cubes');
    expect(overlay.getDebugState().hitRegions.some((region) => region.key === 'badge')).toBe(false);
    expect(overlay.getDebugState().badgeRegions.length).toBe(2);

    const badge = overlay.getDebugState().badgeRegions[0]!;
    const canvasInstance = chromeCanvas({
      canvas: { getBoundingClientRect: () => ({ left: 0, top: 0 }) },
      convertCanvasToOffset: (point: Vec2) => point,
      ds: { scale: 1, offset: [0, 0] },
    });
    const insideBadgeEvent = mouseEvent(badge.rect.x + 2, badge.rect.y + 2);

    const handled = overlay.handlePointerMove(insideBadgeEvent, canvasInstance);

    expect(handled).toBe(false);
    expect(overlay.getDebugState().hoveredBadgeInstance).toBe(metadata.instance_id);
    expect(['name', 'author']).toContain(overlay.getDebugState().hoveredBadgeKey);
    expect(overlay.getDebugState().hoveredKey).toBeNull();
  });

  test('chrome overlay falls back to Unknown source', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-3',
      default_alias: 'Fallback Cube',
      instance_alias: '',
      dirty: false,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 480, 60], metadata);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'Unknown source',
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('chrome overlay uses structured fallback source for malformed cube ids', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay({
      resolveSource: () => ({
        sourceKind: 'github',
        author: 'LegacyAuthor',
        pack: 'LegacyPack',
        namespace: '',
      }),
    });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-malformed-source',
      cube_id: 'not-canonical',
      default_alias: 'Fallback Cube',
      dirty: false,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 420, 60], metadata);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'from LegacyPack by LegacyAuthor',
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('chrome overlay renders local cubes as local source text', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-local',
      cube_id: 'local/personal/text_to_image.cube',
      default_alias: 'Local Cube',
      instance_alias: 'Placed Local Cube',
      dirty: false,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 480, 60], metadata);

    expect(ctx.fillText).toHaveBeenCalledWith(
      'Personal Cube',
      expect.any(Number),
      expect.any(Number),
    );
    expect(ctx.fillText).not.toHaveBeenCalledWith(
      'by local',
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('chrome overlay routes defaults through implementation save', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const actions = {
      onSaveImplementation: jest.fn(),
      onSaveAuthoredFlavor: jest.fn(),
      onSaveLocalFlavor: jest.fn(),
      onManageFlavors: jest.fn(),
      onFlavorChange: jest.fn(),
    };
    const overlay = new CubeChromeOverlay({ actions });
    const metadata = { cube_id: 'cube-1', dirty: true };

    const options = overlay.buildMenuOptions({
      metadata,
      isDirty: true,
      flavors: [
        { id: 'default', name: 'Base', scope: 'authored' },
        { id: 'alt', name: 'Alt', scope: 'local' },
      ],
    });
    const titles = options.map((entry) => entry.title);

    expect(titles).toEqual(['Save cube implementation']);
    expect(titles.some((title) => /flavor/i.test(title))).toBe(false);

    options[0].callback();

    expect(actions.onSaveImplementation).toHaveBeenCalledWith(metadata);
    expect(actions.onSaveAuthoredFlavor).not.toHaveBeenCalled();
    expect(actions.onSaveLocalFlavor).not.toHaveBeenCalled();
    expect(actions.onManageFlavors).not.toHaveBeenCalled();
    expect(actions.onFlavorChange).not.toHaveBeenCalled();
  });

  test('chrome overlay keeps menu actions available when clean', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const actions = {
      onSaveImplementation: jest.fn(),
    };
    const overlay = new CubeChromeOverlay({ actions });
    const metadata = { cube_id: 'cube-2', dirty: false };

    const options = overlay.buildMenuOptions({
      metadata,
      isDirty: false,
      flavors: [],
    });

    expect(options.map((entry) => entry.title)).toEqual(['Save cube implementation']);
  });

  test('chrome overlay replaces definition save with explicit read-only workflow actions', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const actions = {
      onSaveImplementation: jest.fn(),
      getLibraryClassification: jest.fn(() => ({
        definitionId: 'definition-2',
        cubeId: 'cube-2',
        cubeVersion: '1.0.0',
        semanticHash: 'a'.repeat(64),
        instanceIds: ['instance-2'],
        primaryClass: 'none' as const,
        access: 'read_only' as const,
        sourceAvailable: true,
        permittedOperations: new Set(['keep', 'capture', 'track_source', 'fork']),
      })),
      onKeepWorkflowCube: jest.fn(),
      onCaptureWorkflowCube: jest.fn(),
      onSyncWorkflowCubeSource: jest.fn(),
      onForkWorkflowCube: jest.fn(),
    };
    const overlay = new CubeChromeOverlay({ actions });
    const metadata = { cube_id: 'cube-2', instance_id: 'instance-2', dirty: false };

    const options = overlay.buildMenuOptions({ metadata, isDirty: false, flavors: [] });

    expect(options.map((entry) => entry.title)).toEqual([
      'Keep workflow copy',
      'Capture Cube',
      'Synchronize home source…',
      'Fork to Local Cubes…',
    ]);
    options.forEach((entry) => entry.callback());
    expect(actions.onSaveImplementation).not.toHaveBeenCalled();
    expect(actions.onKeepWorkflowCube).toHaveBeenCalledWith(metadata);
    expect(actions.onCaptureWorkflowCube).toHaveBeenCalledWith(metadata);
    expect(actions.onSyncWorkflowCubeSource).toHaveBeenCalledWith(metadata);
    expect(actions.onForkWorkflowCube).toHaveBeenCalledWith(metadata);
  });

  test('chrome overlay opens LiteGraph context menu from menu pill', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const overlay = new CubeChromeOverlay({ adapter, actions: {} });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-menu',
      default_alias: 'Menu Cube',
      dirty: true,
      flavor_options: [{ id: 'default', name: 'Base', scope: 'authored' }],
    };
    globalThis.LiteGraph.ContextMenu = jest.fn();

    overlay.renderHeader(canvasContext(ctx), [0, 0, 360, 60], metadata);
    const menuRegion = overlay.getDebugState().hitRegions.find((region) => region.key === 'menu');
    expect(menuRegion).toBeTruthy();

    const canvasInstance = chromeCanvas({
      canvas: { getBoundingClientRect: () => ({ left: 0, top: 0 }) },
      convertCanvasToOffset: (point: Vec2) => point,
      ds: { scale: 1, offset: [0, 0] },
    });
    if (!menuRegion) throw new Error('Missing menu region');
    const event = mouseEvent(menuRegion.rect.x + 2, menuRegion.rect.y + 2);

    const handled = overlay.handleMouseDown(event, canvasInstance);

    expect(handled).toBe(true);
    expect(globalThis.LiteGraph.ContextMenu).toHaveBeenCalledWith(expect.any(Array), { event });
  });

  test('chrome overlay menu no-ops when LiteGraph context menu missing', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const overlay = new CubeChromeOverlay({ adapter, actions: {} });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-menu-2',
      default_alias: 'Menu Cube',
      dirty: true,
    };
    delete globalThis.LiteGraph.ContextMenu;

    overlay.renderHeader(canvasContext(ctx), [0, 0, 360, 60], metadata);
    const menuRegion = overlay.getDebugState().hitRegions.find((region) => region.key === 'menu');
    expect(menuRegion).toBeTruthy();

    const canvasInstance = chromeCanvas({
      canvas: { getBoundingClientRect: () => ({ left: 0, top: 0 }) },
      convertCanvasToOffset: (point: Vec2) => point,
      ds: { scale: 1, offset: [0, 0] },
    });
    if (!menuRegion) throw new Error('Missing menu region');
    const event = mouseEvent(menuRegion.rect.x + 2, menuRegion.rect.y + 2);

    const handled = overlay.handleMouseDown(event, canvasInstance);

    expect(handled).toBe(true);
  });

  test('overlay manager exposes no separate cube-default action', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const toast = { push: jest.fn() };
    const saveService = { saveImplementation: jest.fn() };
    const manager = new OverlayManager({
      adapter: { getApp: () => ({ graph: {} }) },
      saveService,
      toast,
    });

    expect(manager.getChromeDebugState().actions).not.toHaveProperty('onSaveCubeDefaults');
    expect(manager.getChromeDebugState().actions.onSaveAuthoredFlavor).toBeUndefined();
    expect(manager.getChromeDebugState().actions.onSaveLocalFlavor).toBeUndefined();
    expect(manager.getChromeDebugState().actions.onManageFlavors).toBeUndefined();
    expect(manager.getChromeDebugState().actions.onFlavorChange).toBeUndefined();

    expect(toast.push).not.toHaveBeenCalled();
  });

  test('overlay manager retains the save service receiver for Cube implementation saves', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const saveService = {
      receiver: null as unknown,
      savedCubeIds: [] as string[],
      saveImplementation(
        this: { receiver: unknown; savedCubeIds: string[] },
        options: {
          cubeIds: string[];
        },
      ): void {
        this.receiver = this;
        this.savedCubeIds = options.cubeIds;
      },
    };
    const manager = new OverlayManager({ saveService });

    manager.getChromeDebugState().actions.onSaveImplementation?.({
      cube_id: 'local/personal/Saveable.cube',
    });

    expect(saveService.receiver).toBe(saveService);
    expect(saveService.savedCubeIds).toEqual(['local/personal/Saveable.cube']);
  });

  test('overlay manager forwards the live graph summary for draft Cube saves', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const saveDraft = jest.fn();
    const manager = new OverlayManager({ saveDraft });
    const graphSummary = {
      nodeIds: ['node-1', 'node-2'],
      markerIds: ['input-1', 'output-1'],
      inputCount: 1,
      outputCount: 1,
    };

    manager.getChromeDebugState().actions.onSaveDraft?.({
      kind: 'draft',
      instance_id: 'draft-1',
      graphSummary,
    });

    expect(saveDraft).toHaveBeenCalledWith('draft-1', graphSummary);
  });

  test('chrome overlay does not render or trigger flavor action pill', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const actions = {
      onFlavorChange: jest.fn(),
    };
    const overlay = new CubeChromeOverlay({ adapter, actions });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-actions',
      default_alias: 'Action Cube',
      dirty: true,
      flavor_options: [
        { id: 'default', name: 'Base', scope: 'authored' },
        { id: 'alt', name: 'Alt', scope: 'local' },
      ],
      flavor: 'default',
      flavor_scope: 'authored',
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 420, 60], metadata);

    expect(overlay.getDebugState().hitRegions.some((entry) => entry.key === 'flavor')).toBe(false);
    expect(actions.onFlavorChange).not.toHaveBeenCalled();
  });

  test('chrome overlay swap buttons trigger layout actions', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const actions = {
      onSwapLeft: jest.fn(),
      onSwapRight: jest.fn(),
      canSwap: () => true,
    };
    const overlay = new CubeChromeOverlay({ adapter, actions });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-swap',
      default_alias: 'Swap Cube',
      dirty: false,
      markers: { inputs: [1], outputs: [2] },
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 420, 60], metadata);

    const canvasInstance = chromeCanvas({
      canvas: { getBoundingClientRect: () => ({ left: 0, top: 0 }) },
      convertCanvasToOffset: (point: Vec2) => point,
      ds: { scale: 1, offset: [0, 0] },
    });
    const clickRegion = (key: string) => {
      const region = overlay.getDebugState().hitRegions.find((entry) => entry.key === key);
      if (!region) throw new Error(`Missing ${key} region`);
      const event = mouseEvent(region.rect.x + 2, region.rect.y + 2);
      return overlay.handleMouseDown(event, canvasInstance);
    };

    const menuRegion = overlay.getDebugState().hitRegions.find((entry) => entry.key === 'menu');
    const rightRegion = overlay
      .getDebugState()
      .hitRegions.find((entry) => entry.key === 'swap-right');
    expect(menuRegion).toBeTruthy();
    expect(rightRegion).toBeTruthy();
    if (!menuRegion || !rightRegion) throw new Error('Missing ordered chrome regions');
    expect(menuRegion.rect.x).toBeGreaterThan(rightRegion.rect.x);

    expect(clickRegion('swap-left')).toBe(true);
    expect(clickRegion('swap-right')).toBe(true);
    expect(actions.onSwapLeft).toHaveBeenCalledWith(metadata);
    expect(actions.onSwapRight).toHaveBeenCalledWith(metadata);
  });

  test('chrome overlay hides swap buttons when cube lacks input/output markers', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const adapter = chromeAdapter();
    const actions = {
      onSwapLeft: jest.fn(),
      onSwapRight: jest.fn(),
      canSwap: () => false,
    };
    const overlay = new CubeChromeOverlay({ adapter, actions });
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      scale: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const metadata = {
      managed: true,
      instance_id: 'instance-no-io',
      default_alias: 'Swapless',
      dirty: false,
      markers: { inputs: [], outputs: [] },
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 420, 60], metadata);

    expect(overlay.getDebugState().hitRegions.some((region) => region.key === 'swap-left')).toBe(
      false,
    );
    expect(overlay.getDebugState().hitRegions.some((region) => region.key === 'swap-right')).toBe(
      false,
    );
  });

  test('overlay manager swap uses computed gap and origin', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const adapter = { getApp: () => ({ graph: {} }) };
    const layoutService: NonNullable<OverlayManagerOptions['layoutService']> = {
      buildIndex: () => ({}),
      deriveOrder: () => [
        {
          instanceId: 'a',
          bounds: { x: 10, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [1], outputs: [2] },
        },
        {
          instanceId: 'b',
          bounds: { x: 60, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [3], outputs: [4] },
        },
      ],
      swapOrder: jest.fn(),
    };
    const manager = new OverlayManager({ adapter, layoutService });

    manager.swapLayout({ instance_id: 'a' }, 1);

    expect(layoutService.swapOrder).toHaveBeenCalledWith({
      graph: adapter.getApp().graph,
      aId: 'a',
      bId: 'b',
      order: [
        {
          instanceId: 'a',
          bounds: { x: 10, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [1], outputs: [2] },
        },
        {
          instanceId: 'b',
          bounds: { x: 60, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [3], outputs: [4] },
        },
      ],
      layout: {
        origin: [10, 5],
        gaps: [30],
        minGap: 24,
      },
    });
  });

  test('overlay manager swap scopes neighbor selection to anchored chain order', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const adapter = { getApp: () => ({ graph: {} }) };
    const index = {};
    const globalOrder = [
      {
        instanceId: 'a',
        bounds: { x: 0, y: 5, w: 20, h: 10 },
        markerLookup: { inputs: [1], outputs: [2] },
      },
      {
        instanceId: 'c',
        bounds: { x: 30, y: 5, w: 20, h: 10 },
        markerLookup: { inputs: [5], outputs: [6] },
      },
      {
        instanceId: 'b',
        bounds: { x: 80, y: 5, w: 20, h: 10 },
        markerLookup: { inputs: [3], outputs: [4] },
      },
    ];
    const anchoredOrder = [globalOrder[0], globalOrder[2]];
    const deriveOrder = jest.fn((_index: unknown, options: { anchorInstanceId?: string } = {}) => {
      return options?.anchorInstanceId === 'a' ? anchoredOrder : globalOrder;
    });
    const layoutService: NonNullable<OverlayManagerOptions['layoutService']> = {
      buildIndex: () => index,
      deriveOrder,
      swapOrder: jest.fn(),
    };
    const manager = new OverlayManager({ adapter, layoutService });

    manager.swapLayout({ instance_id: 'a' }, 1);

    expect(deriveOrder).toHaveBeenCalledWith(
      index,
      expect.objectContaining({
        graph: adapter.getApp().graph,
        anchorInstanceId: 'a',
      }),
    );
    expect(layoutService.swapOrder).toHaveBeenCalledWith({
      graph: adapter.getApp().graph,
      aId: 'a',
      bId: 'b',
      order: [
        {
          instanceId: 'a',
          bounds: { x: 0, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [1], outputs: [2] },
        },
        {
          instanceId: 'b',
          bounds: { x: 80, y: 5, w: 20, h: 10 },
          markerLookup: { inputs: [3], outputs: [4] },
        },
      ],
      layout: {
        origin: [0, 5],
        gaps: [60],
        minGap: 24,
      },
    });
  });

  test('overlay manager passes proximity matches into anchored order strategy', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const graph = {};
    const adapter = { getApp: () => ({ graph }) };
    const index = {};
    const matches = [
      {
        outputId: 1,
        inputId: 2,
        outputNode: { graph },
        inputNode: { graph },
      },
    ];
    const layoutService: NonNullable<OverlayManagerOptions['layoutService']> = {
      buildIndex: () => index,
      deriveOrder: jest.fn(() => [
        {
          instanceId: 'a',
          bounds: { x: 0, y: 0, w: 20, h: 10 },
          markerLookup: { inputs: [1], outputs: [2] },
        },
        {
          instanceId: 'b',
          bounds: { x: 30, y: 0, w: 20, h: 10 },
          markerLookup: { inputs: [3], outputs: [4] },
        },
      ]),
      swapOrder: jest.fn(),
    };
    const manager = new OverlayManager({ adapter, layoutService });
    manager.proximity.settings = {
      enabled: true,
      radius: 160,
      strict: true,
      showOverlay: true,
    };
    manager.proximity.overlayMatches = matches as ProximityMatch[];

    manager.swapLayout({ instance_id: 'a' }, 1);

    expect(layoutService.deriveOrder).toHaveBeenCalledWith(
      index,
      expect.objectContaining({
        graph,
        anchorInstanceId: 'a',
        proximityMatches: matches,
      }),
    );
  });

  test('overlay manager swap ignores cubes without input/output markers', async () => {
    await loadUi();
    const { OverlayManager } = await import(
      '../../../frontend/comfyui/ui/overlays/OverlayManager.js'
    );
    const adapter = { getApp: () => ({ graph: {} }) };
    const layoutService: NonNullable<OverlayManagerOptions['layoutService']> = {
      buildIndex: () => ({}),
      deriveOrder: () => [
        {
          instanceId: 'a',
          markerLookup: { inputs: [], outputs: [] },
          bounds: { x: 0, y: 0, w: 10, h: 10 },
        },
        {
          instanceId: 'b',
          markerLookup: { inputs: [1], outputs: [2] },
          bounds: { x: 20, y: 0, w: 10, h: 10 },
        },
      ],
      swapOrder: jest.fn(),
    };
    const manager = new OverlayManager({ adapter, layoutService });

    manager.swapLayout({ instance_id: 'a' }, 1);

    expect(layoutService.swapOrder).not.toHaveBeenCalled();
  });

  test('chrome overlay renders cube icon for menu pill', async () => {
    await loadUi();
    const { CubeChromeOverlay } = await import(
      '../../../frontend/comfyui/ui/overlays/CubeChromeOverlay.js'
    );
    const overlay = new CubeChromeOverlay();
    const ctx = {
      font: '',
      textBaseline: '',
      fillStyle: '',
      save: jest.fn(),
      restore: jest.fn(),
      beginPath: jest.fn(),
      closePath: jest.fn(),
      moveTo: jest.fn(),
      lineTo: jest.fn(),
      arcTo: jest.fn(),
      quadraticCurveTo: jest.fn(),
      rect: jest.fn(),
      translate: jest.fn(),
      rotate: jest.fn(),
      fill: jest.fn(),
      stroke: jest.fn(),
      arc: jest.fn(),
      measureText: (text: string) => ({ width: text.length * 6 }),
      fillText: jest.fn(),
    };
    const drawCubeIconSpy = jest.spyOn(overlay, 'drawCubeIcon');
    const metadata = {
      managed: true,
      instance_id: 'instance-menu-icon',
      default_alias: 'Menu Icon Cube',
      dirty: false,
    };

    overlay.renderHeader(canvasContext(ctx), [0, 0, 360, 60], metadata);

    expect(drawCubeIconSpy).toHaveBeenCalled();
  });

  test('proximity overlay matches compatible Cube node boundaries', () => {
    const adapter = {
      getLiteGraph: () => ({ isValidConnection: () => true }),
      getConsole: () => console,
    };
    const overlay = new ProximityOverlay({ adapter });
    overlay.setEndpointSource({
      discover: () => ({
        outputs: [
          {
            key: 'cube-output',
            endpointId: 'cube-output',
            slot: 0,
            cube: 'out-definition',
            instanceId: 'output-container',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [80, 20],
            slotName: 'image',
            originId: 'cube@output-container|inner-output',
            originSlot: 0,
          },
        ],
        inputs: [
          {
            key: 'cube-input',
            endpointId: 'cube-input',
            slot: 0,
            cube: 'in-definition',
            instanceId: 'input-container',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [120, 20],
            slotName: 'image',
            promptTargets: [
              {
                nodeId: 'cube@input-container|inner-input',
                inputSlot: 0,
                inputName: 'image',
              },
            ],
          },
        ],
      }),
    });

    const matches = overlay.computeMatches(createTestGraph(), { radius: 200, strict: true });

    expect(matches.length).toBe(1);
    expect(matches[0]!.outputId).toBe('cube-output');
    expect(matches[0]!.inputId).toBe('cube-input');
  });

  test('prompt-effective proximity routes survive visual overlay refreshes', () => {
    const overlay = new ProximityOverlay();
    const match: ProximityMatch = {
      outputId: 10,
      outputSlot: 0,
      outputPos: [0, 0],
      inputId: 20,
      inputSlot: 0,
      inputName: 'image',
      inputPos: [10, 0],
      originId: '10:producer',
      originSlot: 1,
      promptTargets: [{ nodeId: '20:consumer', inputSlot: 0, inputName: 'image' }],
      distance: 10,
    };
    jest.spyOn(overlay, 'computeMatches').mockReturnValue([match]);

    const result = overlay.applyProximityToPrompt({
      output: {
        '10:producer': { inputs: {}, class_type: 'Producer' },
        '20:consumer': { inputs: {}, class_type: 'Consumer' },
      },
    });

    expect(result).toEqual({
      output: {
        '10:producer': { inputs: {}, class_type: 'Producer' },
        '20:consumer': {
          inputs: { image: ['10:producer', 1] },
          class_type: 'Consumer',
        },
      },
    });
    expect(overlay.promptMatches).toEqual([match]);

    overlay.updateOverlay([]);

    expect(overlay.overlayMatches).toEqual([]);
    expect(overlay.promptMatches).toEqual([match]);

    overlay.settings.enabled = false;
    overlay.applyProximityToPrompt(result);

    expect(overlay.promptMatches).toEqual([]);
  });

  test('reuses accepted direct-execution proximity without legacy prompt patching', () => {
    const graph = createTestGraph();
    const overlay = new ProximityOverlay();
    const match: ProximityMatch = {
      outputId: 10,
      outputSlot: 0,
      outputPos: [0, 20],
      inputId: 20,
      inputSlot: 0,
      inputName: 'image',
      inputPos: [80, 20],
      originId: 'producer',
      originSlot: 0,
      promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
      distance: 80,
    };
    const computeMatches = jest.spyOn(overlay, 'computeMatches');
    overlay.updateOverlay([match]);

    expect(overlay.promptMatches).toEqual([]);
    expect(overlay.resolveExecutionMatches(graph)).toEqual([match]);
    expect(computeMatches).not.toHaveBeenCalled();
  });

  test('does not invalidate presentation for an unchanged proximity result', () => {
    const setDirty = jest.fn();
    const updateMatches = jest.fn();
    const overlay = new ProximityOverlay({
      adapter: {
        getApp: () => ({ canvas: { setDirty } }),
      },
    });
    overlay.setMatchSink({ updateMatches });
    updateMatches.mockClear();
    const match: ProximityMatch = {
      outputId: 10,
      outputSlot: 0,
      outputPos: [0, 20],
      inputId: 20,
      inputSlot: 0,
      inputName: 'image',
      inputPos: [80, 20],
      originId: 'producer',
      originSlot: 0,
      promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
      distance: 80,
    };

    overlay.updateOverlay([match]);
    overlay.updateOverlay([{ ...match }]);

    expect(updateMatches).toHaveBeenCalledTimes(1);
    expect(setDirty).toHaveBeenCalledTimes(1);
  });

  test('keeps magnetic matches active when dotted-line visibility is disabled', () => {
    const updateMatches = jest.fn();
    const overlay = new ProximityOverlay();
    overlay.settings.showOverlay = false;
    overlay.setMatchSink({ updateMatches });
    updateMatches.mockClear();
    const match: ProximityMatch = {
      outputId: 10,
      outputSlot: 0,
      outputPos: [0, 20],
      inputId: 20,
      inputSlot: 0,
      inputName: 'image',
      inputPos: [80, 20],
      originId: 'producer',
      originSlot: 0,
      promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
      distance: 80,
    };

    overlay.updateOverlay([match]);

    expect(overlay.isProximityEnabled()).toBe(true);
    expect(overlay.overlayMatches).toEqual([]);
    expect(updateMatches).toHaveBeenCalledWith([match]);
  });

  test('invalidates the visible workflow graph when proximity state changes', () => {
    const setDirtyCanvas = jest.fn();
    const overlay = new ProximityOverlay({
      adapter: {
        getApp: () => ({ canvas: { graph: { setDirtyCanvas } } }),
      },
    });

    overlay.updateOverlay([]);

    expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
  });

  test('initializes an unmatched graph once instead of rediscovering on every draw', () => {
    const graph = createTestGraph();
    const overlay = new ProximityOverlay({
      adapter: {
        getApp: () => ({ graph }),
      },
      scheduler: {
        raf: jest.fn(() => 1),
      },
    });
    const compute = jest.spyOn(overlay, 'computeMatches').mockReturnValue([]);

    overlay.ensurePreview(graph);
    overlay.ensurePreview(graph);
    overlay.ensurePreview(graph);

    expect(compute).toHaveBeenCalledTimes(1);
  });

  test('recomputes a replaced endpoint source against the visible workflow graph', () => {
    const rootGraph = createTestGraph();
    const visibleGraph = createTestGraph();
    const discover = jest.fn(() => ({ outputs: [], inputs: [] }));
    const overlay = new ProximityOverlay({
      adapter: {
        getApp: () => ({ graph: rootGraph, canvas: { graph: visibleGraph } }),
      },
    });

    overlay.setEndpointSource({ discover });

    expect(discover).toHaveBeenCalledWith(visibleGraph);
  });

  test('reset allows every previously initialized workflow graph to initialize again', () => {
    const graph = createTestGraph();
    const overlay = new ProximityOverlay();
    const compute = jest.spyOn(overlay, 'computeMatches').mockReturnValue([]);

    overlay.ensurePreview(graph);
    overlay.resetOverlayState();
    overlay.ensurePreview(graph);

    expect(compute).toHaveBeenCalledTimes(2);
  });

  test('proximity overlay does not connect one Cube node to itself', () => {
    const adapter = {
      getLiteGraph: () => ({ isValidConnection: () => true }),
      getConsole: () => console,
    };
    const overlay = new ProximityOverlay({ adapter });
    overlay.setEndpointSource({
      discover: () => ({
        outputs: [
          {
            key: 'output',
            endpointId: 'cube',
            slot: 0,
            cube: 'definition',
            instanceId: 'same-container',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [80, 20],
            slotName: 'image',
            originId: 'cube@same-container|inner',
            originSlot: 0,
          },
        ],
        inputs: [
          {
            key: 'input',
            endpointId: 'cube',
            slot: 0,
            cube: 'definition',
            instanceId: 'same-container',
            alias: 'image',
            type: 'IMAGE',
            slotPos: [0, 20],
            slotName: 'image',
            promptTargets: [
              {
                nodeId: 'cube@same-container|inner',
                inputSlot: 0,
                inputName: 'image',
              },
            ],
          },
        ],
      }),
    });

    const matches = overlay.computeMatches(createTestGraph(), { radius: 200, strict: true });

    expect(matches.length).toBe(0);
  });
});
