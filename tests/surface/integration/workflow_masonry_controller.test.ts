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
import { NativeForwardedWidgetLease } from '../../../frontend/comfyui/ui/surface/masonry/NativeForwardedWidgetLease.js';
import type { NativeNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

describe('workflow masonry controller', () => {
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
    expect(setup.mount).toHaveBeenCalledTimes(6);
    expect(document.querySelector<HTMLElement>('.sugarcubes-workflow-masonry')?.style.opacity).toBe(
      '0',
    );
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    setup.controller.leaveImmediately();
    for (const frame of frames) frame(0);
    await pending;
    expect(setup.mount).toHaveBeenCalledTimes(6);
    expect(setup.unmount).toHaveBeenCalledTimes(6);
    expect(setup.controller.active).toBe(false);
  });
  test('keeps graph visible during the surface fade and safely reverses a pending exit', async () => {
    const setup = fixture();
    const effects: Array<{ finish(): void; cancel: ReturnType<typeof jest.fn> }> = [];
    const mount = setup.mount.getMockImplementation();
    setup.mount.mockImplementation((...args) => {
      const result = mount?.(...args);
      const surface = args[0].closest<HTMLElement>('.sugarcubes-workflow-masonry');
      if (!result || !surface) throw new Error('Native mount surface missing');
      Object.defineProperty(surface, 'animate', {
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
    await Promise.resolve();
    await Promise.resolve();
    expect(effects).toHaveLength(1);
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
    effects[0]?.finish();
    await entry;
    expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
    const exit = setup.controller.toggle();
    expect(setup.container.dataset.sugarcubesMasonry).toBeUndefined();
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
    expect(setup.container.dataset.sugarcubesMasonry).toBe('true');
    setup.controller.leaveImmediately();
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

  test('uses the SugarCubes modal once for active Nodes 1 fallback and preserves edits', async () => {
    const setup = fixture();
    const nativeAlert = jest.spyOn(window, 'alert').mockImplementation(() => undefined);
    await setup.controller.toggle();
    setup.setNodes2(false);
    setup.controller.rendererChanged();
    setup.controller.rendererChanged();
    await setup.controller.toggle();
    expect(setup.alert).toHaveBeenCalledTimes(1);
    expect(setup.alert).toHaveBeenCalledWith(
      'Masonry requires Nodes 2.0',
      'SugarCubes masonry does not support Nodes 1.0. Your workflow is now displayed in graph mode.',
    );
    expect(nativeAlert).not.toHaveBeenCalled();
    expect(setup.controller.active).toBe(false);
    expect(setup.unmount).toHaveBeenCalledTimes(1);
    setup.setNodes2(true);
    setup.controller.rendererChanged();
    await setup.controller.toggle();
    expect(setup.controller.active).toBe(true);
    setup.controller.leaveImmediately();
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

/** Bind a controllable host boundary to the real presentation state machine and view. */
function fixture() {
  const container = document.createElement('div');
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
  const alert = jest
    .fn<(title: string, message: string) => Promise<unknown>>()
    .mockResolvedValue(true);
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
    isNodes2: () => nodes2,
    getSections,
    getRenderer,
    getCardState: () => cardState,
    alert,
    reportError,
    openCube() {},
    onStateChange() {},
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
    alert,
    reportError,
    setNodes2: (value: boolean) => {
      nodes2 = value;
    },
  };
}
