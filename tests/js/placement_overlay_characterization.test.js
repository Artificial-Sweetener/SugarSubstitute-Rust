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
import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { PlacementOverlay } from '../../web/comfyui/ui/overlays/PlacementOverlay.js';

function createPreparedPayload() {
  return {
    cube: { version: '1.2.3' },
    warnings: ['', 'Backend warning'],
    layout: {
      origin: [10, 20],
      groups: [
        {
          bounding: [0, 0, 100, 80],
          properties: { sugarcubes: { instance_alias: 'Preview group' } },
        },
      ],
    },
    nodes: [{ layout: { pos: [20, 30], size: [40, 50] } }],
    markers: [
      { kind: 'input', layout: { pos: [5, 10], size: [20, 25] } },
      { kind: 'output', layout: { pos: [70, 10], size: [20, 25] } },
    ],
  };
}

function createHarness(overrides = {}) {
  const canvasElement = document.createElement('canvas');
  canvasElement.getBoundingClientRect = jest.fn(() => ({
    left: 10,
    top: 20,
    right: 210,
    bottom: 120,
    width: 200,
    height: 100,
  }));
  canvasElement.setPointerCapture = jest.fn();
  canvasElement.releasePointerCapture = jest.fn();
  const graph = {};
  const canvas = {
    canvas: canvasElement,
    graph,
    ds: { scale: 2, offset: [5, 10] },
    last_mouse_position: [30, 50],
    setDirty: jest.fn(),
  };
  const app = { canvas, graph };
  const scheduler = {
    raf: jest.fn(() => 1),
    cancelRaf: jest.fn(),
  };
  const payload = createPreparedPayload();
  const cubeApi = {
    load: jest.fn(async () => ({
      response: { ok: true, statusText: '' },
      data: payload,
    })),
    loadRevision: jest.fn(async () => ({
      response: { ok: true, statusText: '' },
      data: payload,
    })),
    ...overrides.cubeApi,
  };
  const cubeBrowser = {
    setBusy: jest.fn(),
    close: jest.fn(),
  };
  const toast = { push: jest.fn() };
  const applyPreparedImport = jest.fn(async () => ({ success: true, created: [1] }));
  const reportImportOutcome = jest.fn();
  const buildShiftedPlacementPayload = jest.fn((source, shift, origin) => ({
    ...source,
    shiftedBy: shift,
    targetOrigin: origin,
  }));
  const adapter = {
    getApp: () => app,
    getDocument: () => document,
    getWindow: () => window,
    getLiteGraph: () => ({ NODE_TITLE_HEIGHT: 30, NODE_COLLAPSED_WIDTH: 80 }),
    getConsole: () => ({ warn: jest.fn() }),
    ...overrides.adapter,
  };
  const overlay = new PlacementOverlay({
    adapter,
    scheduler,
    cubeApi,
    cubeBrowser,
    toast,
    applyPreparedImport,
    reportImportOutcome,
    buildShiftedPlacementPayload,
  });
  return {
    adapter,
    app,
    applyPreparedImport,
    buildShiftedPlacementPayload,
    canvas,
    canvasElement,
    cubeApi,
    cubeBrowser,
    overlay,
    payload,
    reportImportOutcome,
    scheduler,
    toast,
  };
}

function pointerEvent(overrides = {}) {
  return {
    button: 0,
    pointerId: 9,
    clientX: 50,
    clientY: 80,
    preventDefault: jest.fn(),
    stopPropagation: jest.fn(),
    stopImmediatePropagation: jest.fn(),
    ...overrides,
  };
}

function createDrawingContext() {
  return {
    font: '',
    fillStyle: '',
    strokeStyle: '',
    globalAlpha: 1,
    lineWidth: 1,
    save: jest.fn(),
    restore: jest.fn(),
    setLineDash: jest.fn(),
    fillRect: jest.fn(),
    strokeRect: jest.fn(),
    fillText: jest.fn(),
    measureText: jest.fn(() => ({ width: 40 })),
  };
}

describe('placement overlay characterization', () => {
  beforeEach(() => {
    document.body.replaceChildren();
    const sidebar = document.createElement('div');
    sidebar.className = 'side-bar-panel';
    sidebar.style.display = 'flex';
    document.body.appendChild(sidebar);
  });

  test('coordinate conversion preserves host conversion and transform fallbacks', () => {
    const { canvas, overlay } = createHarness();

    expect(overlay.convertCanvasPoint(canvas, [20, 40])).toEqual([5, 10]);
    expect(overlay.computeOriginFromEvent(pointerEvent())).toEqual([15, 20]);
    expect(overlay.isPointerOverCanvas(pointerEvent())).toBe(true);
    expect(overlay.isPointerOverCanvas(pointerEvent({ clientX: 500 }))).toBe(false);

    canvas.convertCanvasToOffset = jest.fn(([x, y]) => [x + 1, y + 2]);
    expect(overlay.convertCanvasPoint(canvas, [20, 40])).toEqual([21, 42]);
    expect(overlay.computeDropOrigin()).toEqual([31, 52]);
  });

  test('current placement loads at a neutral origin and installs graph interaction', async () => {
    const { canvas, cubeApi, cubeBrowser, overlay, scheduler, toast } = createHarness();

    await overlay.start('  local/example-user/demo.cube  ', {
      defaultAlias: '  Demo  ',
      closeBrowser: true,
      origin: [50, 60],
    });

    const [rawBody, requestOptions] = cubeApi.load.mock.calls[0];
    expect(JSON.parse(rawBody)).toEqual({
      cube_id: 'local/example-user/demo.cube',
      origin: { x: 0, y: 0 },
    });
    expect(requestOptions).toEqual({ headers: { 'Content-Type': 'application/json' } });
    expect(cubeApi.loadRevision).not.toHaveBeenCalled();
    expect(overlay.state).toMatchObject({
      active: true,
      cubeId: 'local/example-user/demo.cube',
      defaultAlias: 'Demo',
      baseOrigin: [10, 20],
      origin: [50, 60],
      cubeVersion: '1.2.3',
      cubeRevisionRef: 'WORKTREE',
    });
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);
    expect(cubeBrowser.setBusy.mock.calls).toEqual([[true], [false]]);
    expect(cubeBrowser.close).toHaveBeenCalledTimes(1);
    expect(toast.push).toHaveBeenCalledWith(
      'info',
      'Place SugarCube',
      'Click on the canvas to place it. Press Esc to cancel.',
    );
    expect(document.body.classList.contains('sugarcubes-placement--active')).toBe(true);
    expect(document.querySelector('.side-bar-panel').style.display).toBe('none');
    expect(scheduler.raf).toHaveBeenCalled();

    overlay.stop();
  });

  test('historical placement uses the revision endpoint and explicit version', async () => {
    const { cubeApi, overlay } = createHarness();

    await overlay.start('local/example-user/demo.cube', {
      revisionRef: 'abc123',
      version: ' 2.0.0 ',
    });

    const [rawBody] = cubeApi.loadRevision.mock.calls[0];
    expect(JSON.parse(rawBody)).toEqual({
      cube_id: 'local/example-user/demo.cube',
      revision_ref: 'abc123',
      origin: { x: 0, y: 0 },
    });
    expect(cubeApi.load).not.toHaveBeenCalled();
    expect(overlay.state.cubeRevisionRef).toBe('abc123');
    expect(overlay.state.cubeVersion).toBe('2.0.0');

    overlay.stop();
  });

  test('placement load failures preserve backend message and detail', async () => {
    const { cubeBrowser, overlay, toast } = createHarness({
      cubeApi: {
        load: jest.fn(async () => ({
          response: { ok: false, statusText: 'Bad Gateway' },
          data: { error: { message: 'Cube unavailable', detail: 'Checkout missing' } },
        })),
      },
    });

    await overlay.start('local/example-user/demo.cube');

    expect(overlay.state.active).toBe(false);
    expect(toast.push).toHaveBeenCalledWith('error', 'Cube unavailable', 'Checkout missing');
    expect(cubeBrowser.setBusy.mock.calls).toEqual([[true], [false]]);
  });

  test('commit shifts the prepared payload, reports warnings, and closes after success', async () => {
    const {
      applyPreparedImport,
      buildShiftedPlacementPayload,
      cubeBrowser,
      overlay,
      payload,
      reportImportOutcome,
    } = createHarness();
    overlay.state.active = true;
    overlay.state.cubeId = 'local/example-user/demo.cube';
    overlay.state.defaultAlias = 'Demo';
    overlay.state.payload = payload;
    overlay.state.baseOrigin = [10, 20];
    overlay.state.origin = [40, 70];

    await overlay.commit();

    expect(buildShiftedPlacementPayload).toHaveBeenCalledWith(payload, [30, 50], [40, 70]);
    const shiftedPayload = buildShiftedPlacementPayload.mock.results[0].value;
    expect(applyPreparedImport).toHaveBeenCalledWith(shiftedPayload, {
      instanceAlias: 'Demo',
      dropOrigin: [40, 70],
    });
    expect(reportImportOutcome).toHaveBeenCalledWith(
      'Demo',
      ['Backend warning'],
      { success: true, created: [1] },
      shiftedPayload,
      { focus: false },
    );
    expect(cubeBrowser.setBusy.mock.calls).toEqual([[true], [false]]);
    expect(cubeBrowser.close).toHaveBeenCalledTimes(1);
    expect(overlay.state.active).toBe(false);
  });

  test('left click commits once and right click cancels active placement', async () => {
    const { canvas, canvasElement, overlay } = createHarness();
    overlay.state.active = true;
    const commit = jest.spyOn(overlay, 'commit').mockResolvedValue();
    const leftClick = pointerEvent();

    expect(overlay.handlePlacementMouseDown(leftClick, canvas, canvasElement)).toBe(true);
    expect(leftClick.cancelBubble).toBe(true);
    expect(canvasElement.setPointerCapture).toHaveBeenCalledWith(9);
    expect(commit).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(overlay.state.commitInProgress).toBe(false);

    overlay.state.active = true;
    const rightClick = pointerEvent({ button: 2 });
    expect(overlay.handlePlacementMouseDown(rightClick, canvas, canvasElement)).toBe(true);
    expect(overlay.state.active).toBe(false);
    expect(rightClick.cancelBubble).toBe(true);
  });

  test('render draws group, node, input, and output previews at shifted positions', () => {
    const { canvas, overlay, payload } = createHarness();
    const ctx = createDrawingContext();
    overlay.state.active = true;
    overlay.state.payload = payload;
    overlay.state.defaultAlias = 'Demo';
    overlay.state.baseOrigin = [10, 20];
    overlay.state.origin = [30, 50];

    overlay.render(ctx, canvas);

    expect(ctx.fillRect).toHaveBeenCalledTimes(4);
    expect(ctx.strokeRect).toHaveBeenCalledTimes(4);
    expect(ctx.fillRect).toHaveBeenNthCalledWith(1, 30, 50, 100, 80);
    expect(ctx.fillRect).toHaveBeenNthCalledWith(2, 40, 60, 40, 50);
    expect(ctx.fillRect).toHaveBeenNthCalledWith(3, 25, 40, 20, 25);
    expect(ctx.fillRect).toHaveBeenNthCalledWith(4, 90, 40, 20, 25);
    expect(ctx.fillText).toHaveBeenCalledWith('Preview group', 33, 57);
  });
});
