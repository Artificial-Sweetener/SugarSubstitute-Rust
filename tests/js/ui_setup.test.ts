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
import { describe, expect, test, beforeEach } from '@jest/globals';
import { app } from './mocks/app.js';
import type { MockSidebarTab } from './mocks/app.js';

function setupCanvas() {
  return {
    graph: { onNodeAdded: () => {}, onNodeRemoved: () => {}, onNodeConnectionChange: () => {} },
    onAfterChange: () => {},
    onDrawForeground: () => {},
    onDrawBackground: () => {},
    processMouseMove: () => {},
    processMouseDown: () => {},
    setDirty: () => {},
  };
}

async function loadUi() {
  const cacheBust = `?v=${Math.random().toString(36).slice(2)}`;
  return import(`../../web/comfyui/ui.js${cacheBust}`);
}

beforeEach(() => {
  app.reset();
  app.graph = {};
  app.canvas = setupCanvas();
  app.extensionManager = { registerSidebarTab: () => {} };
  app.clean = () => {};

  globalThis.requestAnimationFrame = (callback) => {
    callback(0);
    return 1;
  };
  globalThis.cancelAnimationFrame = () => {};

  globalThis.window = {
    SugarCubes: {} as SugarCubesPublicApi,
    setInterval: () => 1,
    clearInterval: () => {},
    comfyAPI: { vueApp: { config: { globalProperties: { $toast: null } } } },
  } as unknown as Window & typeof globalThis;
  globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;
  globalThis.window.cancelAnimationFrame = globalThis.cancelAnimationFrame;
  class LGraphCanvas {}
  class LGraphGroup {}
  globalThis.LiteGraph = {
    LGraphCanvas,
    LGraphGroup,
    LinkDirection: { LEFT: 3, RIGHT: 4 },
  } as unknown as LiteGraphHost;
  globalThis.LiteGraph.LGraphCanvas!.prototype.drawConnections = () => {};
  globalThis.LiteGraph.LGraphCanvas!.prototype.drawForeground = () => {};
});

describe('ui setup and hooks', () => {
  test('setup registers sidebar tab when extension manager is available', async () => {
    const registerCalls: MockSidebarTab[] = [];
    app.extensionManager.registerSidebarTab = (payload) => registerCalls.push(payload);

    const module = await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    expect(registerCalls).toHaveLength(1);
    expect(registerCalls[0].title).toBe('SugarCubes');
    expect(typeof module).toBe('object');
  });

  test('graph hooks trigger refresh scheduling on canvas changes', async () => {
    const rafCalls: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCalls.push(callback);
      callback(0);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    app.canvas.onAfterChange();
    expect(rafCalls.length).toBeGreaterThan(0);
  });

  test('clean hook wraps app.clean without throwing', async () => {
    let cleaned = false;
    app.clean = () => {
      cleaned = true;
    };

    await loadUi();
    const extension = app._extensions[0];
    await extension.setup!();

    app.clean();
    expect(cleaned).toBe(true);
  });
});
