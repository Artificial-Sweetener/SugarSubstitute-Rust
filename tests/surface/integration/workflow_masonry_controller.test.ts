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
/** Verify reversible native presentation, edits, cancellation, and renderer fallback. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryController } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryController.js';
import type { WorkflowMasonryControllerOptions } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryController.js';
import { WorkflowMasonryLinkMotion } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryLinkMotion.js';
import { WorkflowMasonryTransition } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryTransition.js';
import { NATIVE_PRESENTATION_READY_EVENT } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';
import { NativeCardPresentationOwner } from '../../../frontend/comfyui/ui/surface/NativeCardPresentationOwner.js';
import { WorkflowMasonryView } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryView.js';
import { MasonryStackInsetMotion } from '../../../frontend/comfyui/ui/surface/masonry/MasonryStackInsetMotion.js';
import { NativeForwardedWidgetLease } from '../../../frontend/comfyui/ui/surface/NativeForwardedWidgetLease.js';
import type { NativeNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('rebinds a replaced Cube face as one source lookup and one presenter refresh', async () => {
  const ready = jest.fn();
  const setup = fixture(undefined, { onSurfaceReady: ready });
  const cube: CubeNode = {
    id: 99,
    pos: [0, 0],
    size: [400, 300],
    inputs: [],
    outputs: [],
    connect: () => undefined,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    properties: { sugarcubes_cube: { instance_id: 'batch-cube' } },
    subgraph: {
      id: 'batch-definition',
      name: 'Batch cube',
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add: () => undefined,
      remove: () => undefined,
      addInput: () => ({ connect: () => undefined }),
      addOutput: () => ({ connect: () => undefined }),
      configure: () => undefined,
    },
  };
  const section = setup.sections[0]!;
  section.cube = cube;
  const template = section.cards[0]!;
  section.cards = [1, 2, 3].map((id) => ({
    ...template,
    id: `cube:99:${String(id)}`,
    node: { ...setup.node, id },
  }));
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.nodeId = '99';
  const cells = section.cards.map((card) => {
    const cell = document.createElement('div');
    cell.className = 'sugarcubes-cube-face__node-card';
    cell.dataset.cubeNodeId = String(card.node.id);
    const wrapper = document.createElement('div');
    wrapper.className = 'sugarcubes-cube-face__native-card-mount';
    wrapper.append(document.createElement('input'));
    cell.append(wrapper);
    return cell;
  });
  root.append(...cells);
  setup.container.append(root);
  const owner = new NativeCardPresentationOwner(jest.fn());
  owner.reset(cells);
  await setup.controller.toggle();
  expect(ready).toHaveBeenCalledTimes(1);
  ready.mockClear();
  setup.getSections.mockClear();
  owner.reset(cells);
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  expect(document.querySelectorAll('.sugarcubes-workflow-masonry input')).toHaveLength(3);
  expect(ready).toHaveBeenCalledTimes(1);
  expect(setup.getSections).not.toHaveBeenCalled();
  expect(setup.mount).not.toHaveBeenCalled();
  setup.controller.leaveImmediately();
  expect(root.querySelectorAll('input')).toHaveLength(3);
  owner.dispose();
});

test('restores a workflow view without animation and ignores identical restoration requests', async () => {
  const setup = fixture();
  const transition = jest.spyOn(WorkflowMasonryTransition.prototype, 'run');
  await setup.controller.restore(true);
  expect(setup.controller.active).toBe(true);
  const surface = document.querySelector('.sugarcubes-workflow-masonry');
  expect(setup.getSections).toHaveBeenCalledTimes(1);
  expect(transition).toHaveBeenLastCalledWith(expect.anything(), true, true);
  transition.mockClear();
  await setup.controller.restore(true);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBe(surface);
  expect(transition).not.toHaveBeenCalled();
  await setup.controller.restore(false);
  expect(setup.controller.active).toBe(false);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(transition).not.toHaveBeenCalled();
});

test('suppresses an incoming graph before loading from graph view and throughout masonry preparation', async () => {
  const setup = fixture();
  let acquire: ((renderer: NativeNodeCardRenderer) => void) | undefined;
  setup.getRenderer.mockImplementation(
    () =>
      new Promise((resolve) => {
        acquire = resolve;
      }),
  );
  const loading = setup.controller.setSuspended('configure', true);
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  await loading;
  await setup.controller.restore(true);
  const restoring = setup.controller.setSuspended('configure', false);
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  acquire?.(setup.renderer);
  await restoring;
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  expect(
    document
      .querySelector('.sugarcubes-workflow-masonry')
      ?.hasAttribute('data-sc-masonry-preparing'),
  ).toBe(false);
  setup.controller.leaveImmediately();
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
});

test('releases loading suppression only when the committed destination is graph', async () => {
  const setup = fixture();
  await setup.controller.restore(true);
  await setup.controller.setSuspended('configure', true);
  await setup.controller.restore(false);
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  await setup.controller.setSuspended('configure', false);
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
});

test('applies stack reservation before the first measured masonry layout', async () => {
  const setup = fixture(undefined, { onSurfaceReady: (view) => view.setStackWidth(224) });
  const layout = jest.spyOn(WorkflowMasonryView.prototype, 'layout');
  await setup.controller.restore(true);
  expect(layout).toHaveBeenCalledTimes(1);
  setup.controller.leaveImmediately();
});

test('reserves the stack before mounting controls at their final column width', async () => {
  const setup = fixture(undefined, {
    onSurfaceMounting: (view) => {
      const content = view.element.querySelector<HTMLElement>(
        '.sugarcubes-workflow-masonry__content',
      );
      if (!content) throw new Error('Masonry content missing');
      Object.defineProperty(content, 'clientWidth', {
        get: () =>
          800 -
          Number.parseFloat(view.element.style.getPropertyValue('--sc-masonry-stack-width') || '0'),
      });
      view.setStackWidth(224);
    },
  });
  const mount = setup.mount.getMockImplementation();
  if (!mount) throw new Error('Native mount missing');
  setup.mount.mockImplementation((...args) => {
    expect(args[0].closest<HTMLElement>('.sugarcubes-workflow-masonry__card')?.style.width).toBe(
      '576px',
    );
    return mount(...args);
  });
  await setup.controller.restore(true);
  expect(setup.mount).toHaveBeenCalledTimes(1);
  expect(setup.reportError).not.toHaveBeenCalled();
  setup.controller.leaveImmediately();
});

test('releases handoff suppression when the destination renderer fails', async () => {
  const setup = fixture();
  await setup.controller.setSuspended('configure', true);
  await setup.controller.restore(true);
  setup.getRenderer.mockRejectedValueOnce(new Error('Renderer failed'));
  await setup.controller.setSuspended('configure', false);
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
  expect(setup.reportError).toHaveBeenCalledTimes(1);
  expect(setup.controller.active).toBe(false);
});

test('acquires controls immediately after a committed configuration without another frame barrier', async () => {
  const setup = fixture();
  await setup.controller.setSuspended('configure', true);
  await setup.controller.restore(true);
  const restoring = setup.controller.setSuspended('configure', false);
  expect(setup.getRenderer).toHaveBeenCalledTimes(1);
  await restoring;
  setup.controller.leaveImmediately();
});

test('keeps restored geometry instantaneous until a user interaction enables layout motion', async () => {
  let view: WorkflowMasonryView | undefined;
  let width = 400;
  const setup = fixture(undefined, {
    onSurfaceMounting: (surface) => {
      view = surface;
      const content = surface.element.querySelector('.sugarcubes-workflow-masonry__content');
      if (!content) throw new Error('Masonry content missing');
      Object.defineProperty(content, 'clientWidth', { get: () => width });
    },
  });
  const section = setup.sections[0]!;
  section.cards.push({ ...section.cards[0]!, id: 'root:2', node: { ...setup.node, id: 2 } });
  await setup.controller.restore(true);
  if (!view) throw new Error('Restored view missing');
  const effects: Keyframe[][] = [];
  for (const cell of view.cards().values())
    Object.defineProperty(cell, 'animate', {
      value: (frames: Keyframe[]) => {
        effects.push(frames);
        return { finished: Promise.resolve(), cancel() {} };
      },
    });
  width = 1000;
  view.layout();
  expect(effects).toHaveLength(0);
  document.dispatchEvent(new Event('pointerdown'));
  width = 400;
  view.layout();
  expect(effects.length).toBeGreaterThan(0);
  setup.controller.leaveImmediately();
});

test('restores workflow scroll and rebinds controls without replaying entry motion', async () => {
  const setup = fixture();
  const transition = jest.spyOn(WorkflowMasonryTransition.prototype, 'run');
  await setup.controller.toggle();
  const initial = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry');
  if (!initial) throw new Error('Masonry fixture is missing');
  initial.scrollTop = 240;
  transition.mockClear();
  await setup.controller.setSuspended('configure', true);
  expect(setup.controller.presenting).toBe(true);
  expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
  setup.node.widgets![0]!.value = 'restored';
  await setup.controller.setSuspended('configure', false);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).not.toBe(initial);
  expect(document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry')?.scrollTop).toBe(240);
  expect(
    document.querySelector<HTMLInputElement>('.sugarcubes-workflow-masonry input')?.value,
  ).toBe('restored');
  expect(transition).toHaveBeenCalledTimes(1);
  expect(transition).toHaveBeenLastCalledWith(expect.anything(), true, true);
  expect(
    document
      .querySelector('.sugarcubes-workflow-masonry')
      ?.hasAttribute('data-sc-masonry-preparing'),
  ).toBe(false);
  setup.controller.leaveImmediately();
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
});

test('keeps scroll positions scoped to workflow identities and releases suppression for app mode', async () => {
  const setup = fixture();
  const first = {};
  const second = {};
  setup.setScope(first);
  await setup.controller.toggle();
  const surface = (): HTMLElement => {
    const element = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry');
    if (!element) throw new Error('Masonry fixture is missing');
    return element;
  };
  surface().scrollTop = 240;
  await setup.controller.setSuspended('configure', true);
  setup.setScope(second);
  await setup.controller.setSuspended('configure', false);
  expect(surface().scrollTop).toBe(0);
  surface().scrollTop = 90;
  await setup.controller.setSuspended('configure', true);
  setup.setScope(first);
  await setup.controller.setSuspended('configure', false);
  expect(surface().scrollTop).toBe(240);
  await setup.controller.setSuspended('configure', true);
  await setup.controller.setSuspended('app', true);
  expect(setup.controller.presenting).toBe(false);
  expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
  setup.controller.leaveImmediately();
});

/** Retain requested masonry and recover automatically when native assets become reachable. */
test('recovers a transient native asset failure without an unavailable toast or second toggle', async () => {
  const setup = fixture();
  const { ComfyRuntimeAssetUnavailableError } = await import(
    '../../../frontend/comfyui/ui/surface/ComfyRuntimeAssetRequest.js'
  );
  jest.useFakeTimers();
  jest.spyOn(console, 'warn').mockImplementation(() => undefined);
  try {
    setup.getRenderer.mockRejectedValueOnce(
      new ComfyRuntimeAssetUnavailableError(
        '/assets/GraphView.js',
        new TypeError('Failed to fetch'),
      ),
    );
    const pending = setup.controller.toggle();
    await jest.advanceTimersByTimeAsync(0);
    expect(setup.controller.active).toBe(true);
    expect(setup.reportError).not.toHaveBeenCalled();
    await jest.advanceTimersByTimeAsync(700);
    await pending;
    expect(document.querySelector('.sugarcubes-workflow-masonry input')).not.toBeNull();
    expect(setup.reportError).not.toHaveBeenCalled();
    expect(setup.getRenderer).toHaveBeenCalledTimes(2);
    setup.controller.leaveImmediately();
  } finally {
    setup.controller.leaveImmediately();
    jest.useRealTimers();
  }
});

describe('workflow masonry controller', () => {
  test('packs preparation once after host styling rather than reflowing every mount delivery', async () => {
    const setup = fixture();
    const callbacks: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      callbacks.push(callback);
      return callbacks.length;
    });
    const layout = jest.spyOn(WorkflowMasonryView.prototype, 'layout');
    const pending = setup.controller.toggle();
    await Promise.resolve();
    await Promise.resolve();
    expect(layout).not.toHaveBeenCalled();
    for (let index = 0; index < callbacks.length; index += 1) {
      callbacks[index]?.(index * 16);
      await Promise.resolve();
    }
    await pending;
    expect(layout).toHaveBeenCalledTimes(1);
    setup.controller.leaveImmediately();
  });
  test('reuses identified Cube controls and returns them instead of mounting a duplicate view', async () => {
    jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    const setup = fixture();
    const cube: CubeNode = {
      id: 99,
      pos: [0, 0],
      size: [400, 300],
      inputs: [],
      outputs: [],
      connect: () => undefined,
      isSubgraphNode: () => true,
      serialize: () => ({}),
      properties: { sugarcubes_cube: { instance_id: 'borrowed-cube' } },
      subgraph: {
        id: 'borrowed-definition',
        name: 'Borrowed cube',
        _nodes: [],
        inputs: [],
        outputs: [],
        inputNode: {},
        outputNode: {},
        add: () => undefined,
        remove: () => undefined,
        addInput: () => ({ connect: () => undefined }),
        addOutput: () => ({ connect: () => undefined }),
        configure: () => undefined,
      },
    };
    const root = document.createElement('div');
    root.className = 'lg-node';
    root.dataset.nodeId = '99';
    const face = document.createElement('div');
    face.className = 'sugarcubes-cube-face';
    const cell = document.createElement('div');
    cell.className = 'sugarcubes-cube-face__node-card';
    cell.dataset.cubeNodeId = String(setup.node.id);
    cell.style.cssText = 'position:absolute;left:25px;top:40px;width:300px;height:120px';
    const wrapper = document.createElement('div');
    wrapper.className = 'sugarcubes-cube-face__native-card-mount';
    const input = document.createElement('input');
    input.value = 'Original widget';
    wrapper.append(input);
    cell.append(wrapper);
    face.append(cell);
    root.append(face);
    setup.container.append(root);
    const owner = new NativeCardPresentationOwner(jest.fn());
    owner.reset([cell]);
    const measureSources = jest.spyOn(owner, 'measureSources');
    const originalMeasure = measureSources.getMockImplementation();
    if (!originalMeasure) throw new Error('Source measurement fixture missing');
    measureSources.mockImplementation(() => {
      expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
      originalMeasure.call(owner);
    });
    const section = setup.sections[0]!;
    section.cube = cube;
    section.cards[0]!.id = 'cube:99:1';
    for (let visit = 0; visit < 3; visit += 1) {
      await setup.controller.toggle();
      expect(document.querySelector('.sugarcubes-workflow-masonry input')).toBe(input);
      expect(setup.mount).not.toHaveBeenCalled();
      expect(face.querySelector('input')).toBeNull();
      expect(owner.borrowed).toBe(true);
      await setup.controller.toggle();
      expect(face.querySelector('input')).toBe(input);
      expect(cell.style.width).toBe('300px');
      expect(cell.style.left).toBe('25px');
      expect(owner.borrowed).toBe(false);
      expect(document.querySelector('[data-sc-card-origin]')).toBeNull();
      expect(setup.unmount).not.toHaveBeenCalled();
    }
    expect(measureSources).toHaveBeenCalledTimes(3);
    measureSources.mockRestore();
    await setup.controller.toggle();
    const surface = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry');
    const presented = surface?.querySelector<HTMLElement>('.sugarcubes-workflow-masonry__card');
    const header = surface?.querySelector<HTMLElement>('.sugarcubes-workflow-masonry__section');
    if (!surface || !presented || !header) throw new Error('Missing borrowed presentation');
    presented.style.height = '301px';
    surface.dispatchEvent(new Event('sugarcubes-disclosure-layout'));
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(header.dataset.masonryHeight).toBe('345');
    expect(header.style.getPropertyValue('--sc-masonry-section-height')).toBe('345px');
    presented.style.height = '182px';
    owner.reset([cell]);
    queueMicrotask(() => {
      presented.style.height = '301px';
    });
    await Promise.resolve();
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(header.dataset.masonryHeight).toBe('345');
    await setup.controller.toggle();
    owner.dispose();
  });

  test('suppresses graph interaction at layer boundaries and restores preexisting inert state', async () => {
    const setup = fixture();
    const root = document.createElement('div');
    root.id = 'graph-canvas-container';
    setup.container.replaceWith(root);
    root.append(setup.container);
    const layers = ['ph-no-capture', 'isolate'].map((className) => {
      const element = document.createElement('div');
      element.className = className;
      element.inert = false;
      root.append(element);
      return element;
    });
    layers[1]!.inert = true;
    const mount = setup.mount.getMockImplementation();
    if (!mount) throw new Error('Native mount fixture missing');
    setup.mount.mockImplementation((...args) => {
      expect(layers.map((element) => element.inert)).toEqual([true, true]);
      return mount(...args);
    });
    await setup.controller.toggle();
    expect(layers.map((element) => element.inert)).toEqual([true, true]);
    await setup.controller.toggle();
    expect(layers.map((element) => element.inert)).toEqual([false, true]);
    await setup.controller.toggle();
    setup.controller.leaveImmediately();
    expect(layers.map((element) => element.inert)).toEqual([false, true]);
    expect(root.dataset.sugarcubesMasonry).toBeUndefined();
  });
  test('keeps ancestor capture forwarding out of masonry scrolling and restores graph zoom on exit', async () => {
    const setup = fixture();
    let graphScale = 1;
    const canvas = document.createElement('canvas');
    document.body.append(canvas);
    canvas.addEventListener('wheel', (event) => {
      graphScale *= event.deltaY > 0 ? 0.9 : 1.1;
    });
    setup.container.addEventListener(
      'wheel',
      (event) => {
        const target = event.target;
        // Comfy forwards during ancestor capture unless the target opts into native scrolling.
        if (target instanceof Element && target.closest('[data-capture-wheel="true"]')) return;
        event.preventDefault();
        event.stopPropagation();
        canvas.dispatchEvent(new WheelEvent('wheel', { deltaY: event.deltaY }));
      },
      { capture: true },
    );
    await setup.controller.toggle();
    const surface = document.querySelector('.sugarcubes-workflow-masonry');
    const input = surface?.querySelector('input');
    if (!surface || !input) throw new Error('Masonry native controls missing');
    for (const target of [surface, input]) {
      for (const modifiers of [{}, { ctrlKey: true }, { metaKey: true }]) {
        for (const deltaY of [120, -120]) {
          const event = new WheelEvent('wheel', {
            bubbles: true,
            cancelable: true,
            deltaY,
            ...modifiers,
          });
          target.dispatchEvent(event);
          expect(event.defaultPrevented).toBe(false);
          expect(graphScale).toBe(1);
        }
      }
    }
    await setup.controller.toggle();
    canvas.dispatchEvent(new WheelEvent('wheel', { deltaY: 120 }));
    expect(graphScale).toBe(0.9);
  });
  test('retains native controls and focus when another card joins the section', async () => {
    const setup = fixture();
    await setup.controller.toggle();
    const input = document.querySelector('input');
    if (!input) throw new Error('Native input missing');
    input.focus();
    const section = setup.sections[0];
    const first = section?.cards[0];
    if (!section || !first) throw new Error('Card missing');
    section.cards.push({ ...first, id: 'root:2', node: { ...setup.node, id: 2 } });
    await setup.controller.graphChanged();
    expect(document.querySelector('input')).toBe(input);
    expect(document.activeElement).toBe(input);
    expect(setup.mount).toHaveBeenCalledTimes(2);
    expect(setup.unmount).not.toHaveBeenCalled();
    const cells = [...document.querySelectorAll<HTMLElement>('[data-masonry-card]')];
    expect(cells.map((cell) => cell.dataset.scProjectionOrder)).toEqual(['0', '1']);
    section.cards.reverse();
    await setup.controller.graphChanged();
    expect(cells.map((cell) => cell.dataset.scProjectionOrder)).toEqual(['1', '0']);
    expect(document.querySelector('input')).toBe(input);
    expect(setup.mount).toHaveBeenCalledTimes(2);
    setup.controller.leaveImmediately();
  });
  test('reuses the optional-node menu and activation switch while keeping the live section and scroll surface', async () => {
    const setup = fixture();
    setup.node.mode = 4;
    setup.node.title = '<img src=x>';
    const graph = { _nodes: [setup.node] };
    setup.getSections.mockImplementation(() => projectWorkflowMasonry(graph, []));
    const unsubscribe = setup.cardState.subscribe(() => {
      void setup.controller.graphChanged();
    });
    await setup.controller.toggle();
    const surface = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry')!;
    const header = surface.querySelector('header');
    expect(surface.querySelectorAll('[data-masonry-card]')).toHaveLength(0);
    surface.scrollTop = 123;
    surface.querySelector<HTMLButtonElement>('[data-sugarcubes-card-visibility-button]')!.click();
    const menu = document.querySelector('[data-sugarcubes-card-visibility-menu]');
    expect(menu).not.toBeNull();
    expect(menu?.textContent).toContain('<img src=x>');
    expect(menu?.querySelector('img')).toBeNull();
    menu?.querySelector<HTMLElement>('[role="menuitemcheckbox"]')!.click();
    for (let index = 0; index < 12; index += 1) await Promise.resolve();
    expect(document.querySelector('[data-sugarcubes-card-visibility-menu]')).toBe(menu);
    expect(surface.querySelector('header')).toBe(header);
    expect(surface.scrollTop).toBe(123);
    expect(surface.querySelectorAll('[data-masonry-card]')).toHaveLength(1);
    expect(
      window.getComputedStyle(surface.querySelector<HTMLElement>('[data-masonry-card]')!).position,
    ).toBe('absolute');
    const activation = surface.querySelector<HTMLInputElement>('[role="switch"]')!;
    expect(activation.checked).toBe(false);
    expect(setup.node.mode).toBe(4);
    activation.checked = true;
    activation.dispatchEvent(new Event('change'));
    for (let index = 0; index < 12; index += 1) await Promise.resolve();
    expect(setup.node.mode).toBe(0);
    expect(surface.querySelector<HTMLInputElement>('[role="switch"]')?.checked).toBe(true);
    expect(surface.querySelector('[role="switch"]')).toBe(activation);
    expect(document.querySelector('[data-sugarcubes-card-visibility-menu]')).toBe(menu);
    await setup.controller.toggle();
    await setup.controller.toggle();
    expect(document.querySelector<HTMLInputElement>('[role="switch"]')?.checked).toBe(true);
    setup.controller.leaveImmediately();
    unsubscribe();
    expect(document.querySelector('[data-sugarcubes-card-visibility-menu]')).toBeNull();
  });

  test('assigns final column widths before mounting and releases every card between visits', async () => {
    const setup = fixture();
    const mount = setup.mount.getMockImplementation();
    const widths: string[] = [];
    setup.mount.mockImplementation((...args) => {
      const cell = args[0].closest<HTMLElement>('[data-masonry-card]');
      widths.push(cell?.style.width ?? '');
      const result = mount?.(...args);
      if (!result) throw new Error('Native mount missing');
      return result;
    });
    for (let visit = 0; visit < 3; visit += 1) {
      await setup.controller.toggle();
      expect(document.querySelectorAll('.sugarcubes-workflow-masonry')).toHaveLength(1);
      await setup.controller.toggle();
      expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
      expect(setup.unmount).toHaveBeenCalledTimes(visit + 1);
    }
    expect(widths).toHaveLength(3);
    expect(widths.every((width) => Number.parseFloat(width) > 0)).toBe(true);
  });

  test('yields native mount batches and cancels the remaining work on immediate exit', async () => {
    const setup = fixture();
    const frames: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const section = setup.sections[0];
    if (!section) throw new Error('Section missing');
    section.cards = Array.from({ length: 20 }, (_, index) => ({
      id: String(index),
      node: { ...setup.node, id: index },
      columnSpan: 1,
      label: 'Example',
      visible: true,
      enabled: true,
      showActivationControl: false,
    }));
    const pending = setup.controller.toggle();
    await Promise.resolve();
    await Promise.resolve();
    expect(setup.mount).toHaveBeenCalledTimes(6);
    expect(
      document
        .querySelector('.sugarcubes-workflow-masonry')
        ?.hasAttribute('data-sc-masonry-preparing'),
    ).toBe(true);
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    setup.controller.leaveImmediately();
    for (const frame of frames) frame(0);
    await pending;
    expect(setup.mount).toHaveBeenCalledTimes(6);
    expect(setup.unmount).toHaveBeenCalledTimes(6);
    expect(setup.controller.active).toBe(false);
  });
  test('keeps graph visible during assembly and safely reverses a pending exit', async () => {
    const leaving = jest.fn();
    const setup = fixture(undefined, {
      onSurfaceReady: (view) => view.setStackWidth(248),
      onSurfaceLeaving: leaving,
    });
    const effects: Array<{ finish(): void; cancel: ReturnType<typeof jest.fn> }> = [];
    const mount = setup.mount.getMockImplementation();
    setup.mount.mockImplementation((...args) => {
      const result = mount?.(...args);
      const surface = args[0].closest<HTMLElement>('.sugarcubes-workflow-masonry');
      if (!result || !surface) throw new Error('Native mount surface missing');
      const backdrop = surface.previousElementSibling;
      if (!backdrop) throw new Error('Masonry backdrop missing');
      Object.defineProperty(backdrop, 'animate', {
        value: () => {
          const completion: { resolve?: () => void; reject?: (error: Error) => void } = {};
          const finished = new Promise<void>((resolve, reject) => {
            completion.resolve = resolve;
            completion.reject = reject;
          });
          const effect = {
            finished,
            finish: () => completion.resolve?.(),
            cancel: jest.fn(() => completion.reject?.(new Error('Cancelled'))),
          };
          effects.push(effect);
          return effect;
        },
      });
      return result;
    });
    const entry = setup.controller.toggle();
    for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
    expect(effects).toHaveLength(1);
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    const movingSurface = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry');
    if (!movingSurface) throw new Error('Masonry surface missing');
    const wheel = new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: 120 });
    movingSurface.dispatchEvent(wheel);
    expect(wheel.defaultPrevented).toBe(true);
    effects[0]?.finish();
    await entry;
    expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
    movingSurface.scrollTop = 123;
    const scrollTo = jest.fn();
    movingSurface.scrollTo = scrollTo;
    const exit = setup.controller.toggle();
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
    expect(effects).toHaveLength(2);
    expect(leaving).not.toHaveBeenCalled();
    expect(scrollTo).toHaveBeenCalledWith({ top: 123, behavior: 'instant' });
    const surface = document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry');
    if (!surface) throw new Error('Masonry surface missing');
    surface.style.opacity = '0.6';
    const reversal = setup.controller.toggle();
    await Promise.resolve();
    await exit;
    expect(setup.unmount).not.toHaveBeenCalled();
    expect(effects).toHaveLength(3);
    effects[2]?.finish();
    await reversal;
    expect(leaving).not.toHaveBeenCalled();
    expect(surface.style.getPropertyValue('--sc-masonry-stack-width')).toBe('248px');
    expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
    setup.controller.leaveImmediately();
    expect(leaving).toHaveBeenCalledTimes(1);
    expect(setup.unmount).toHaveBeenCalledTimes(1);
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
  });
  test('forwards native source DOM edits into the promoted Cube value and releases listeners', async () => {
    const textarea = document.createElement('textarea');
    const source = { name: 'text', value: 'original', element: textarea };
    const promoted = {
      name: 'prompt',
      value: 'original',
      element: document.createElement('textarea'),
    };
    const node: ComfyNode = { widgets: [source] };
    const owner: ComfyNode = { widgets: [promoted], onWidgetChanged: jest.fn() };
    textarea.addEventListener('input', () => {
      source.value = textarea.value;
    });
    const lease = new NativeForwardedWidgetLease(textarea, node, [
      { node: owner, name: 'prompt', targetName: 'text' },
    ]);
    textarea.value = 'edited';
    textarea.dispatchEvent(new Event('input'));
    await Promise.resolve();
    await Promise.resolve();
    expect(promoted.value).toBe('edited');
    expect(owner.onWidgetChanged).toHaveBeenCalledWith('prompt', 'edited', 'original', promoted);
    lease.dispose();
    textarea.value = 'graph edit';
    textarea.dispatchEvent(new Event('input'));
    expect(promoted.value).toBe('edited');
  });
  test('mounts real node references, preserves literal labels and edited values, and releases every mount', async () => {
    const setup = fixture();
    const before = JSON.stringify(setup.node);
    await setup.controller.toggle();
    const wheelAtHost = jest.fn();
    setup.container.addEventListener('wheel', wheelAtHost);
    const surface = document.querySelector('.sugarcubes-workflow-masonry');
    const wheel = new WheelEvent('wheel', { bubbles: true, cancelable: true });
    surface?.dispatchEvent(wheel);
    expect(wheelAtHost).not.toHaveBeenCalled();
    expect(wheel.defaultPrevented).toBe(false);
    expect(setup.mount).toHaveBeenCalledWith(expect.any(HTMLElement), setup.node, {
      allowCollapse: true,
      allowRename: false,
      presentationGeometry: true,
      preserveNativeBody: true,
      widgetSources: [],
    });
    expect(document.querySelector('h2')?.textContent).toBe('<img src=x onerror=bad()>');
    expect(document.querySelector('img')).toBeNull();
    expect(JSON.stringify(setup.node)).toBe(before);
    const input = document.querySelector('input');
    if (!input) throw new Error('Native input is missing');
    input.value = 'edited value';
    input.dispatchEvent(new Event('input'));
    await setup.controller.toggle();
    expect(setup.node.widgets?.[0]?.value).toBe('edited value');
    expect(setup.unmount).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    expect(setup.controller.active).toBe(false);
  });

  test('cancels pending native acquisition when toggled off or renderer changes', async () => {
    const setup = fixture();
    const resolver: { resolve?: (renderer: NativeNodeCardRenderer) => void } = {};
    const deferred = new Promise<NativeNodeCardRenderer>((resolve) => {
      resolver.resolve = resolve;
    });
    setup.getRenderer.mockReturnValue(deferred);
    const pending = setup.controller.toggle();
    await setup.controller.toggle();
    if (!resolver.resolve) throw new Error('Renderer resolver missing');
    resolver.resolve(setup.renderer);
    await pending;
    expect(setup.mount).not.toHaveBeenCalled();
    expect(setup.controller.active).toBe(false);
  });

  test('notifies once without blocking active Nodes 1 fallback and preserves edits', async () => {
    const setup = fixture();
    const nativeAlert = jest.spyOn(window, 'alert').mockImplementation(() => undefined);
    await setup.controller.toggle();
    setup.setNodes2(false);
    setup.controller.rendererChanged();
    setup.controller.rendererChanged();
    await setup.controller.toggle();
    expect(setup.notifyRendererFallback).toHaveBeenCalledTimes(1);
    expect(setup.notifyRendererFallback).toHaveBeenCalledWith();
    expect(document.querySelector('.sugarcubes-modal-overlay')).toBeNull();
    expect(nativeAlert).not.toHaveBeenCalled();
    expect(setup.controller.active).toBe(false);
    expect(setup.unmount).toHaveBeenCalledTimes(1);
    setup.setNodes2(true);
    setup.controller.rendererChanged();
    await setup.controller.toggle();
    expect(setup.controller.active).toBe(true);
    setup.controller.leaveImmediately();
  });

  test('keeps graph fallback complete when its notification adapter fails', async () => {
    const setup = fixture();
    const failure = new Error('Toast transport unavailable.');
    setup.notifyRendererFallback.mockImplementation(() => {
      throw failure;
    });
    await setup.controller.toggle();
    setup.setNodes2(false);
    expect(() => setup.controller.rendererChanged()).not.toThrow();
    expect(setup.controller.active).toBe(false);
    expect(setup.unmount).toHaveBeenCalledTimes(1);
    expect(setup.reportError).toHaveBeenCalledWith(failure);
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
    expect(document.querySelector('.sugarcubes-modal-overlay')).toBeNull();
  });

  test('recovers graph presentation and reports native mount failure', async () => {
    const setup = fixture();
    setup.mount.mockImplementation(() => {
      throw new Error('Unsupported native renderer');
    });
    await setup.controller.toggle();
    expect(setup.reportError).toHaveBeenCalledWith(expect.any(Error));
    expect(setup.controller.active).toBe(false);
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  });

  test('restores a singleton custom widget to its original graph parent', async () => {
    const setup = fixture();
    const parent = document.createElement('div');
    const custom = document.createElement('textarea');
    const sibling = document.createElement('span');
    parent.append(custom, sibling);
    document.body.append(parent);
    setup.node.widgets = [{ name: 'custom', element: custom }];
    setup.mount.mockImplementation((target) => {
      target.append(custom);
      return { refresh() {}, unmount: () => custom.remove() };
    });
    await setup.controller.toggle();
    expect(custom.closest('.sugarcubes-workflow-masonry')).not.toBeNull();
    setup.controller.leaveImmediately();
    expect(custom.parentElement).toBe(parent);
    expect(custom.nextSibling).toBe(sibling);
  });
});

test('mounts every masonry region in its explicitly allocated content parent and leaves sibling controls intact', async () => {
  const f = fixture();
  const workspace = document.createElement('div');
  workspace.id = 'graph-canvas-container';
  const output = document.createElement('button');
  output.textContent = 'Output controls';
  f.container.style.left = '480px';
  f.container.style.width = 'calc(100% - 480px)';
  workspace.append(output, f.container);
  document.body.append(workspace);
  await f.controller.toggle();
  expect(f.container.querySelector('.sugarcubes-workflow-masonry')).not.toBeNull();
  expect(output.parentElement).toBe(workspace);
  expect(output.textContent).toBe('Output controls');
  f.controller.leaveImmediately();
  expect(f.container.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  expect(output.parentElement).toBe(workspace);
  expect(f.container.style.left).toBe('480px');
});

/** Bind a controllable host boundary to the real presentation state machine and view. */
function fixture(
  createLinkMotion?: WorkflowMasonryControllerOptions['createLinkMotion'],
  surfaceLifecycle: Pick<
    WorkflowMasonryControllerOptions,
    'onSurfaceMounting' | 'onSurfaceReady' | 'onSurfaceLeaving' | 'onSurfaceTransition'
  > = {},
) {
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    queueMicrotask(() => callback(0));
    return 0;
  });
  const container = document.createElement('div');
  let scope: object = container;
  document.body.append(container);
  const node: ComfyNode = {
    id: 1,
    type: 'Example',
    pos: [40, 90],
    size: [330, 160],
    widgets: [{ name: 'value', value: 'initial' }],
  };
  const unmount = jest.fn();
  const mount = jest.fn<NativeNodeCardRenderer['mount']>((target, targetNode, options) => {
    const input = document.createElement('input');
    input.value = String(targetNode.widgets?.[0]?.value);
    input.addEventListener('input', () => {
      const widget = targetNode.widgets?.[0];
      if (widget) widget.value = input.value;
    });
    if (options?.headerAccessory) target.append(options.headerAccessory);
    target.append(input);
    return { refresh() {}, unmount };
  });
  const renderer: NativeNodeCardRenderer = { mount, dispose() {} };
  const getRenderer = jest.fn<() => Promise<NativeNodeCardRenderer>>().mockResolvedValue(renderer);
  const notifyRendererFallback = jest.fn<() => void>();
  const reportError = jest.fn();
  let nodes2 = true;
  const sections: WorkflowMasonrySection[] = [
    {
      id: 'loose:1',
      label: '<img src=x onerror=bad()>',
      cube: null,
      target: { _nodes: [node] },
      cards: [
        {
          id: 'root:1',
          node,
          columnSpan: 1,
          label: 'Example',
          visible: true,
          enabled: true,
          showActivationControl: false,
        },
      ],
    },
  ];
  const cardState = new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} });
  const getSections = jest.fn<() => WorkflowMasonrySection[]>().mockImplementation(() => sections);
  const controller = new WorkflowMasonryController({
    document,
    getContainer: () => container,
    getScope: () => scope,
    isNodes2: () => nodes2,
    getSections,
    getRenderer,
    ...(createLinkMotion ? { createLinkMotion } : {}),
    getCardState: () => cardState,
    notifyRendererFallback,
    reportError,
    openCube() {},
    onStateChange() {},
    ...surfaceLifecycle,
  });
  return {
    controller,
    getSections,
    cardState,
    sections,
    node,
    container,
    renderer,
    mount,
    unmount,
    getRenderer,
    getCardState: () => cardState,
    notifyRendererFallback,
    reportError,
    setScope: (value: object) => {
      scope = value;
    },
    setNodes2: (value: boolean) => {
      nodes2 = value;
    },
  };
}

test('coordinates drawer travel with masonry and retains its surface until both finish', async () => {
  let finish: (() => void) | undefined;
  const travel = jest.fn(
    (_entering: boolean) =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const leaving = jest.fn();
  const setup = fixture(undefined, { onSurfaceTransition: travel, onSurfaceLeaving: leaving });
  const entry = setup.controller.toggle();
  for (let tick = 0; tick < 20 && !finish; tick += 1) await Promise.resolve();
  expect(travel).toHaveBeenLastCalledWith(true);
  expect(leaving).not.toHaveBeenCalled();
  finish!();
  await entry;
  finish = undefined;
  const exit = setup.controller.toggle();
  for (let tick = 0; tick < 20 && !finish; tick += 1) await Promise.resolve();
  expect(travel).toHaveBeenLastCalledWith(false);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).not.toBeNull();
  expect(leaving).not.toHaveBeenCalled();
  finish!();
  await exit;
  expect(leaving).toHaveBeenCalledTimes(1);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
});

test.each([82, 248])(
  'retains %ipx rail reservation through exit travel and releases it once at teardown',
  async (width) => {
    let view: WorkflowMasonryView | null = null;
    const leaving = jest.fn(() => view?.setStackWidth(0));
    const setup = fixture(undefined, {
      onSurfaceReady: (surface) => {
        view = surface;
        surface.setStackWidth(width);
      },
      onSurfaceLeaving: leaving,
    });
    let finishExit = (): void => undefined;
    let exitWidth = '';
    jest
      .spyOn(WorkflowMasonryTransition.prototype, 'run')
      .mockImplementation(async (surface, entering) => {
        if (entering) return;
        exitWidth = surface.surface.style.getPropertyValue('--sc-masonry-stack-width');
        await new Promise<void>((resolve) => {
          finishExit = resolve;
        });
      });
    await setup.controller.toggle();
    const exit = setup.controller.toggle();
    for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
    try {
      expect(exitWidth).toBe(`${String(width)}px`);
      expect(leaving).not.toHaveBeenCalled();
      expect(setup.unmount).not.toHaveBeenCalled();
    } finally {
      finishExit();
      await exit;
    }
    expect(leaving).toHaveBeenCalledTimes(1);
    expect(setup.unmount).toHaveBeenCalledTimes(1);
  },
);

test('waits for interrupted stack inset motion before starting exit endpoint measurement', async () => {
  const setup = fixture();
  await setup.controller.toggle();
  let finishInset = (): void => undefined;
  jest.spyOn(MasonryStackInsetMotion.prototype, 'settled').mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finishInset = resolve;
      }),
  );
  const travel = jest.spyOn(WorkflowMasonryTransition.prototype, 'run').mockResolvedValue();
  const exit = setup.controller.toggle();
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
  try {
    expect(travel).not.toHaveBeenCalled();
    expect(setup.unmount).not.toHaveBeenCalled();
  } finally {
    finishInset();
    await exit;
  }
  expect(travel).toHaveBeenCalledTimes(1);
  expect(setup.unmount).toHaveBeenCalledTimes(1);
});

test('retires an exit waiting for inset motion without scheduling more native presentation frames', async () => {
  const setup = fixture();
  await setup.controller.toggle();
  let finishInset = (): void => undefined;
  jest.spyOn(MasonryStackInsetMotion.prototype, 'settled').mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finishInset = resolve;
      }),
  );
  const travel = jest.spyOn(WorkflowMasonryTransition.prototype, 'run').mockResolvedValue();
  const frames = jest.spyOn(window, 'requestAnimationFrame');
  const exit = setup.controller.toggle();
  setup.controller.leaveImmediately();
  const retiredFrames = frames.mock.calls.length;
  finishInset();
  await exit;
  expect(travel).not.toHaveBeenCalled();
  expect(frames).toHaveBeenCalledTimes(retiredFrames);
  expect(setup.unmount).toHaveBeenCalledTimes(1);
});

test('assigns final themed masonry widths before the native renderer mounts controls', async () => {
  const setup = fixture();
  const view = new WorkflowMasonryView({
    document,
    sections: setup.sections,
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  const content = view.element.querySelector('.sugarcubes-workflow-masonry__content');
  if (!content) throw new Error('Masonry content missing');
  Object.defineProperty(content, 'clientWidth', { value: 1100 });
  const prepare = (): void => {
    view.element.style.setProperty('--sugarcubes-masonry-column-width', '400');
    view.element.style.setProperty('--sugarcubes-masonry-gap', '8');
  };
  view.element.addEventListener(NATIVE_PRESENTATION_READY_EVENT, prepare);
  const widths: string[] = [];
  await view.mountCards({
    mount(target) {
      widths.push(target.parentElement?.style.width ?? '');
      return { refresh() {}, unmount() {} };
    },
    dispose() {},
  });
  expect(widths).toEqual(['546px']);
  view.dispose();
});

test('keeps noodles hidden through node travel and recaptures only after native cards return', async () => {
  const events: string[] = [];
  const create = jest.fn((container: HTMLElement) => {
    events.push('capture');
    return new WorkflowMasonryLinkMotion(container, {
      strokes: [],
      restore: () => events.push('restore'),
    });
  });
  const setup = fixture(create);
  setup.renderer.requestSlotLayoutSync = jest.fn(() => events.push('sync-sockets'));
  jest.spyOn(WorkflowMasonryLinkMotion.prototype, 'run').mockImplementation(async (entering) => {
    events.push(entering ? 'unplug' : 'replug');
    if (!entering) expect(setup.unmount).toHaveBeenCalledTimes(1);
  });
  let finish = (): void => undefined;
  jest
    .spyOn(WorkflowMasonryTransition.prototype, 'run')
    .mockImplementation(async (_view, entering) => {
      events.push(entering ? 'cards-enter' : 'cards-exit');
      if (!entering)
        await new Promise<void>((resolve) => {
          finish = resolve;
        });
    });
  await setup.controller.toggle();
  expect(events).toEqual(['sync-sockets', 'capture', 'unplug', 'cards-enter']);
  const hold = jest.spyOn(WorkflowMasonryLinkMotion.prototype, 'hold');
  const exit = setup.controller.toggle();
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
  expect(hold).toHaveBeenCalledWith(true);
  expect(events).toEqual(['sync-sockets', 'capture', 'unplug', 'cards-enter', 'cards-exit']);
  expect(setup.unmount).not.toHaveBeenCalled();
  finish();
  await exit;
  expect(events).toEqual([
    'sync-sockets',
    'capture',
    'unplug',
    'cards-enter',
    'cards-exit',
    'sync-sockets',
    'restore',
    'capture',
    'replug',
    'restore',
  ]);
  expect(document.querySelector('svg')).toBeNull();
});

test('preserves a new masonry request when an old replug completes after reversal', async () => {
  const create = jest.fn(
    (container: HTMLElement) =>
      new WorkflowMasonryLinkMotion(container, { strokes: [], restore: jest.fn() }),
  );
  const setup = fixture(create);
  let finish = (): void => undefined;
  let replugStarted = false;
  jest.spyOn(WorkflowMasonryLinkMotion.prototype, 'run').mockImplementation(async (entering) => {
    if (entering) return;
    replugStarted = true;
    await new Promise<void>((resolve) => {
      finish = resolve;
    });
  });
  await setup.controller.toggle();
  const exit = setup.controller.toggle();
  for (let turn = 0; turn < 30 && !replugStarted; turn += 1) await Promise.resolve();
  expect(replugStarted).toBe(true);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  const entry = setup.controller.toggle();
  finish();
  await exit;
  await entry;
  expect(setup.controller.active).toBe(true);
  expect(document.querySelector('.sugarcubes-workflow-masonry')).not.toBeNull();
  expect(document.querySelector('svg')).not.toBeNull();
  setup.controller.leaveImmediately();
  expect(document.querySelector('svg')).toBeNull();
});

test('drops existing vector effects immediately when reduced motion is enabled during masonry', async () => {
  const media = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  const create = jest.fn(
    (container: HTMLElement) =>
      new WorkflowMasonryLinkMotion(container, { strokes: [], restore: jest.fn() }),
  );
  const setup = fixture(create);
  try {
    await setup.controller.toggle();
    expect(document.querySelector('svg')).not.toBeNull();
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: () => ({ matches: true }),
    });
    await setup.controller.toggle();
    expect(create).toHaveBeenCalledTimes(1);
    expect(document.querySelector('svg')).toBeNull();
    expect(document.querySelector('.sugarcubes-workflow-masonry')).toBeNull();
  } finally {
    setup.controller.leaveImmediately();
    if (media) Object.defineProperty(window, 'matchMedia', media);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
});
