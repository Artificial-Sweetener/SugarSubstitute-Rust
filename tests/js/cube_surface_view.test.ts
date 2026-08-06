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
/** Verify Cube surface composition around Comfy-owned native node cards. */

import { describe, expect, jest, test } from '@jest/globals';

import { CubeSurfaceView } from '../../frontend/comfyui/ui/surface/CubeSurfaceView.js';
import { createDefaultCubeSurfaceState } from '../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import type {
  NativeNodeCardMountOptions,
  NativeNodeCardRenderer,
} from '../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';
import type { CubeIdentityPresentation } from '../../frontend/comfyui/ui/cube/CubeIdentityPresentation.js';
import type { CubePreviewActions } from '../../frontend/comfyui/ui/surface/CubePreviewActions.js';

describe('CubeSurfaceView', () => {
  test('builds one Cube composition with native-card masonry and a preview rail', () => {
    const renderer = createRenderer();
    const state = createDefaultCubeSurfaceState();
    state.minimumColumnWidth = 100;
    state.preview.width = 180;
    const nodes = Array.from({ length: 5 }, (_, index) => createNode(index));
    const view = new CubeSurfaceView({
      document,
      renderer,
      identity: cubeIdentity('Text <script>alert(1)</script>'),
      nodes,
      state,
      onStateChange: jest.fn(),
    });

    view.layout(750);

    expect(view.element.querySelectorAll('[data-cube-node-id]')).toHaveLength(5);
    expect(view.element.querySelector('[data-cube-preview-rail]')).not.toBeNull();
    expect(view.element.querySelector('[data-cube-action="resize"]')).toBeNull();
    expect(view.element.querySelector('[data-cube-port-direction]')).toBeNull();
    expect(view.element.querySelector('[data-cube-masonry]')?.getAttribute('data-columns')).toBe(
      '5',
    );
    expect(view.element.getAttribute('aria-label')).toBe(
      'Text <script>alert(1)</script> Cube contents',
    );
    expect(view.element.querySelector('.sugarcubes-cube-unsaved-indicator')).toBeNull();
    expect(view.element.querySelector('[data-cube-definition-source]')?.textContent).toBe(
      'from Base-Cubes by Artificial-Sweetener',
    );
    expect(view.element.querySelector('script')).toBeNull();
  });

  test('shows a crossed-out native save icon only before the Cube has been saved', () => {
    const identity = cubeIdentity('Draft Cube');
    identity.awaitingFirstSave = true;
    identity.sourceLine = 'Workflow only';
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity,
      nodes: [],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    const indicator = view.element.querySelector<HTMLElement>('.sugarcubes-cube-unsaved-indicator');
    expect(indicator?.getAttribute('aria-label')).toBe('Not saved yet');
    expect(indicator?.querySelector('.pi-save')).not.toBeNull();
    expect(indicator?.querySelector('.pi-ban')).not.toBeNull();
    expect(view.element.querySelector('[data-cube-definition-source]')?.textContent).toBe(
      'Workflow only',
    );
    view.dispose();
  });

  test('does not let legacy persisted card order override the native Cube graph order', () => {
    const state = createDefaultCubeSurfaceState();
    state.nodeOrder = ['2', '1'];
    const models = createNode(1);
    const sampler = createNode(2);
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [models, sampler],
      state,
      onStateChange: jest.fn(),
    });

    expect(
      [...view.element.querySelectorAll<HTMLElement>('[data-cube-node-id]')].map(
        (card) => card.dataset.cubeNodeId,
      ),
    ).toEqual(['1', '2']);
    expect(state.nodeOrder).toEqual(['2', '1']);
    view.dispose();
  });

  test('lays out cards from unscaled native dimensions under canvas zoom', () => {
    const renderer: NativeNodeCardRenderer = {
      mount: jest.fn((target: HTMLElement) => {
        Object.defineProperty(target.parentElement, 'offsetHeight', {
          configurable: true,
          value: 200,
        });
        target.getBoundingClientRect = () => ({ height: 100 }) as unknown as DOMRect;
        return { refresh: jest.fn(), unmount: jest.fn() };
      }),
      dispose: jest.fn(),
    };
    const state = createDefaultCubeSurfaceState();
    state.minimumColumnWidth = 400;
    state.preview.visible = false;
    const view = new CubeSurfaceView({
      document,
      renderer,
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1), createNode(2), createNode(3)],
      state,
      onStateChange: jest.fn(),
    });

    view.layout(300);

    const cells = [...view.element.querySelectorAll<HTMLElement>('[data-cube-node-id]')];
    expect(cells.map((cell) => cell.style.top)).toEqual(['0px', '212px', '424px']);
    expect(view.element.querySelector<HTMLElement>('[data-cube-masonry]')?.style.height).toBe(
      '624px',
    );
    view.dispose();
  });

  test('uses the persisted masonry gap above the first Nodes 2.0 card', () => {
    const state = createDefaultCubeSurfaceState();
    state.gap = 19;
    state.preview.visible = false;
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1), createNode(2)],
      state,
      onStateChange: jest.fn(),
    });

    view.layout(300);

    const content = view.element.querySelector<HTMLElement>('[data-cube-content]');
    const cards = [...view.element.querySelectorAll<HTMLElement>('[data-cube-node-id]')];
    expect(content?.style.getPropertyValue('--sugarcubes-cube-masonry-header-inset')).toBe('19px');
    expect(content?.style.getPropertyValue('--sugarcubes-cube-preview-row-gap')).toBe('19px');
    expect(cards.map((card) => card.style.top)).toEqual(['0px', '119px']);
    view.dispose();
  });

  test('anchors masonry at the top and reports a measured minimum with equal vertical gutters', () => {
    const renderer: NativeNodeCardRenderer = {
      mount: jest.fn((target: HTMLElement) => {
        Object.defineProperty(target.parentElement, 'offsetHeight', {
          configurable: true,
          value: 100,
        });
        return { refresh: jest.fn(), unmount: jest.fn() };
      }),
      dispose: jest.fn(),
    };
    const state = createDefaultCubeSurfaceState();
    state.gap = 19;
    state.minimumColumnWidth = 300;
    state.preview.visible = false;
    const onMinimumHeightChange = jest.fn();
    const view = new CubeSurfaceView({
      document,
      renderer,
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1), createNode(2), createNode(3)],
      state,
      onStateChange: jest.fn(),
      onMinimumHeightChange,
    });
    const content = view.element.querySelector<HTMLElement>('[data-cube-content]');
    if (!content) throw new Error('Missing Cube content.');
    content.scrollTop = 80;
    content.scrollLeft = 12;

    view.layout(300);

    expect(content.scrollTop).toBe(80);
    expect(content.scrollLeft).toBe(12);
    expect(content.style.getPropertyValue('--sugarcubes-cube-masonry-header-inset')).toBe('19px');
    expect(content.style.getPropertyValue('--sugarcubes-cube-masonry-footer-inset')).toBe('19px');
    expect(content.style.getPropertyValue('--sugarcubes-cube-preview-row-gap')).toBe('19px');
    expect(content.style.minHeight).toBe('376px');
    expect(onMinimumHeightChange).toHaveBeenLastCalledWith(376);
    view.dispose();
  });

  test('stacks the preview below masonry until the Cube is wide enough for a rail', () => {
    const state = createDefaultCubeSurfaceState();
    state.minimumColumnWidth = 240;
    state.preview.width = 320;
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Narrow Cube'),
      nodes: [createNode(1)],
      state,
      onStateChange: jest.fn(),
    });

    view.layout(250);

    const masonry = view.element.querySelector<HTMLElement>('[data-cube-masonry]');
    const rail = view.element.querySelector<HTMLElement>('[data-cube-preview-rail]');
    expect(masonry?.style.width).toBe('250px');
    expect(masonry?.dataset.columns).toBe('1');
    expect(rail?.hidden).toBe(false);
    expect(rail?.style.width).toBe('250px');
    expect(rail?.style.height).toBe('160px');
    expect(rail?.style.minHeight).toBe('0px');
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-content]')?.dataset.previewLayout,
    ).toBe('stacked');

    view.layout(900);

    expect(rail?.hidden).toBe(false);
    expect(rail?.style.width).toBe('320px');
    expect(rail?.style.height).toBe('auto');
    expect(rail?.style.minHeight).toBe('0px');
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-content]')?.dataset.previewLayout,
    ).toBe('rail');
  });

  test('renders old Cube chrome actions in the Nodes 2.0 Cube header', () => {
    const onSwapLeft = jest.fn();
    const onSwapRight = jest.fn();
    const onOpenMenu = jest.fn();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('A long Cube title that still needs room'),
      metadata: { instance_id: 'cube-1' },
      chromeActions: {
        onSwapLeft,
        onSwapRight,
        canSwap: (_metadata, direction) => direction === 'left',
        onOpenMenu,
      },
      nodes: [createNode(1)],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    const swapLeft = view.element.querySelector<HTMLButtonElement>(
      '[data-cube-action="swap-left"]',
    );
    const swapRight = view.element.querySelector<HTMLButtonElement>(
      '[data-cube-action="swap-right"]',
    );
    const cubeMenu = view.element.querySelector<HTMLButtonElement>(
      '[data-cube-action="cube-menu"]',
    );
    const cardMenu = view.element.querySelector<HTMLButtonElement>(
      '[data-cube-action="card-menu"]',
    );

    expect(view.element.querySelector('[data-cube-action="edit"]')).toBeNull();
    expect(swapLeft?.querySelector('.pi.pi-arrow-left')).not.toBeNull();
    expect(swapLeft?.textContent).toBe('');
    expect(swapLeft?.hidden).toBe(false);
    expect(swapRight?.hidden).toBe(true);
    expect(cubeMenu?.getAttribute('aria-label')).toBe('Open Cube actions');
    expect(cubeMenu?.hidden).toBe(false);
    expect(cubeMenu?.querySelector('.pi.pi-box')).not.toBeNull();
    expect(cubeMenu?.querySelector('svg')).toBeNull();
    expect(cardMenu?.querySelector('.pi.pi-eye')).not.toBeNull();
    expect(cardMenu?.textContent).toBe('');
    expect(cardMenu?.getAttribute('aria-label')).toBe('Reveal optional Cube cards');
    expect(cardMenu?.hidden).toBe(true);
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-definition-name]')?.textContent,
    ).toBe('SDXL/Text to Image version 2.0.0');
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-definition-source]')?.textContent,
    ).toBe('from Base-Cubes by Artificial-Sweetener');
    expect(view.element.querySelector('.sugarcubes-cube-face__icon img')?.getAttribute('src')).toBe(
      '/cube-icon.png',
    );

    swapLeft?.click();
    cubeMenu?.click();
    expect(onSwapLeft).toHaveBeenCalledWith({ instance_id: 'cube-1' });
    expect(onSwapRight).not.toHaveBeenCalled();
    expect(onOpenMenu).toHaveBeenCalledWith({ instance_id: 'cube-1' }, expect.any(MouseEvent));
  });

  test('always shows the Cube title independently from definition metadata', () => {
    const repeated = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('SDXL/Text to Image'),
      nodes: [],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });
    const aliased = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('My portrait pipeline'),
      nodes: [],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    expect(
      repeated.element.querySelector<HTMLElement>('.sugarcubes-cube-face__title')?.hidden,
    ).toBe(false);
    expect(aliased.element.querySelector<HTMLElement>('.sugarcubes-cube-face__title')?.hidden).toBe(
      false,
    );
    repeated.dispose();
    aliased.dispose();
  });

  test('does not offer ordinary cards through the reveal action', () => {
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1)],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    expect(view.element.querySelectorAll('[data-cube-node-id]')).toHaveLength(1);
    expect(
      view.element.querySelector<HTMLButtonElement>('[data-cube-action="card-menu"]')?.hidden,
    ).toBe(true);
    expect(view.element.querySelector('[data-cube-card-reveal]')).toBeNull();
  });

  test('keeps the card menu action after the header moves into the native node shell', () => {
    const state = createDefaultCubeSurfaceState();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [{ ...createNode(1), mode: 4 }],
      state,
      onStateChange: jest.fn(),
    });
    const nativeHeader = document.createElement('div');
    const nativeRoot = document.createElement('section');
    nativeRoot.className = 'lg-node';
    nativeRoot.append(nativeHeader, view.element);
    nativeHeader.append(view.header);
    const cardMenu = nativeHeader.querySelector<HTMLButtonElement>(
      '[data-cube-action="card-menu"]',
    );

    cardMenu?.click();

    const menu = document.querySelector<HTMLElement>('[data-cube-card-menu]');
    expect(menu?.hidden).toBe(false);
    expect(menu?.closest('.lg-node')).toBeNull();
    expect(menu?.closest('.sugarcubes-cube-face')).toBeNull();
    expect(cardMenu?.getAttribute('aria-expanded')).toBe('true');
    view.dispose();
  });

  test('keeps face geometry invariant through repeated external reveal-menu cycles', () => {
    const renderer: NativeNodeCardRenderer = {
      mount: jest.fn((target: HTMLElement) => {
        Object.defineProperty(target.parentElement, 'offsetHeight', {
          configurable: true,
          value: 100,
        });
        return { refresh: jest.fn(), unmount: jest.fn() };
      }),
      dispose: jest.fn(),
    };
    const optionalNode = createNode(2);
    optionalNode.mode = 4;
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const minimumHeights: number[] = [];
    const view = new CubeSurfaceView({
      document,
      renderer,
      identity: cubeIdentity('Invariant Cube'),
      nodes: [createNode(1), optionalNode],
      state,
      onStateChange: jest.fn(),
      onMinimumHeightChange: (height) => minimumHeights.push(height),
    });
    const nativeHeader = document.createElement('div');
    const nativeRoot = document.createElement('section');
    nativeRoot.className = 'lg-node';
    nativeRoot.append(nativeHeader, view.element);
    nativeHeader.append(view.header);
    const content = view.element.querySelector<HTMLElement>('[data-cube-content]');
    const card = view.element.querySelector<HTMLElement>('[data-cube-node-id="1"]');
    const button = nativeHeader.querySelector<HTMLButtonElement>('[data-cube-action="card-menu"]');
    if (!content || !card || !button) throw new Error('Missing invariant-test Cube elements.');
    content.scrollTop = 47;
    content.scrollLeft = 11;
    view.layout(320);
    const baseline = readFaceGeometry(content, card, minimumHeights);

    for (let cycle = 0; cycle < 3; cycle += 1) {
      button.click();
      const openMenu = document.querySelector<HTMLElement>('[data-cube-card-menu]');
      expect(openMenu?.parentElement).toBe(document.body);
      expect(openMenu?.closest('.lg-node')).toBeNull();
      expect(openMenu?.closest('.sugarcubes-cube-face')).toBeNull();
      view.layout(320);
      expect(readFaceGeometry(content, card, minimumHeights)).toEqual(baseline);

      button.click();
      expect(document.querySelector('[data-cube-card-menu]')).toBeNull();
      view.layout(320);
      expect(readFaceGeometry(content, card, minimumHeights)).toEqual(baseline);
    }

    nativeRoot.style.transform = 'translate(240px, 180px)';
    view.layout(560);
    const resizedBaseline = readFaceGeometry(content, card, minimumHeights);
    button.click();
    button.click();
    view.layout(560);
    expect(readFaceGeometry(content, card, minimumHeights)).toEqual(resizedBaseline);

    view.dispose();
  });

  test('reveals bypassed cards without changing activation, then toggles activation separately', () => {
    const state = createDefaultCubeSurfaceState();
    const bypassed = createNode(7);
    bypassed.mode = 4;
    const onStateChange = jest.fn();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [bypassed],
      state,
      onStateChange,
    });

    expect(view.element.querySelectorAll('[data-cube-node-id]')).toHaveLength(0);
    view.element.querySelector<HTMLButtonElement>('[data-cube-action="card-menu"]')?.click();
    const checkbox = document.querySelector<HTMLInputElement>('[data-cube-card-reveal="7"]');
    if (!checkbox) throw new Error('Missing bypassed card reveal control.');
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));

    expect(bypassed.mode).toBe(4);
    const activation = view.element.querySelector<HTMLElement>('[data-cube-card-activation="7"]');
    const activationInput = activation?.querySelector<HTMLInputElement>('[role="switch"]');
    expect(activation?.textContent).toContain('Disabled');
    expect(activationInput?.getAttribute('aria-checked')).toBe('false');
    if (!activationInput) throw new Error('Missing card activation switch.');
    activationInput.checked = true;
    activationInput.dispatchEvent(new Event('change'));

    expect(bypassed.mode).toBe(0);
    expect(state.cards['7']).toEqual({
      authoredBypass: true,
      revealed: true,
      enabledOverride: true,
      activeMode: 0,
    });
    expect(onStateChange).toHaveBeenCalledTimes(2);
    view.dispose();
  });

  test('leaves boundary slots and resize interactions to the real parent node', () => {
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Connected Cube'),
      nodes: [],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    expect(view.element.querySelector('[data-cube-port-direction]')).toBeNull();
    expect(view.element.querySelector('[data-cube-action="resize"]')).toBeNull();
  });

  test('applies exact conditional gutter widths supplied by the boundary owner', () => {
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Output-only Cube'),
      nodes: [],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    view.setPortGutterWidths(0, 40);

    expect(view.element.style.getPropertyValue('--sugarcubes-cube-input-gutter-width')).toBe('0px');
    expect(view.element.style.getPropertyValue('--sugarcubes-cube-output-gutter-width')).toBe(
      '40px',
    );
    view.dispose();
  });

  test('keeps face preview presentation independent from canonical output sockets', () => {
    const state = createDefaultCubeSurfaceState();
    state.preview.width = 513.6844451311475;
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Empty Cube'),
      nodes: [createNode(1), createNode(2)],
      state,
      onStateChange: jest.fn(),
    });

    view.renderPreview({ outputs: [] });
    view.layout(1_014);

    const masonry = view.element.querySelector<HTMLElement>('[data-cube-masonry]');
    const rail = view.element.querySelector<HTMLElement>('[data-cube-preview-rail]');
    expect(masonry?.dataset.columns).toBe('2');
    expect(Number.parseFloat(masonry?.style.width ?? '')).toBeCloseTo(492.3155548688525);
    expect(rail?.hidden).toBe(false);
    expect(Number.parseFloat(rail?.style.width ?? '')).toBeCloseTo(513.6844451311475);
    expect(rail?.textContent).toContain('No preview available');
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-content]')?.dataset.previewLayout,
    ).toBe('rail');
    view.dispose();
  });

  test('allocates outer frame growth to preview without widening masonry', () => {
    const state = createDefaultCubeSurfaceState();
    const onStateChange = jest.fn();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Expandable preview'),
      nodes: [createNode(1), createNode(2)],
      state,
      onStateChange,
    });

    view.layout(900);
    const masonry = view.element.querySelector<HTMLElement>('[data-cube-masonry]');
    const rail = view.element.querySelector<HTMLElement>('[data-cube-preview-rail]');
    const initialMasonryWidth = Number.parseFloat(masonry?.style.width ?? '');

    view.layout(1_100);

    expect(Number.parseFloat(masonry?.style.width ?? '')).toBe(initialMasonryWidth);
    expect(Number.parseFloat(rail?.style.width ?? '')).toBe(520);
    expect(state.preview.width).toBe(520);
    expect(onStateChange).toHaveBeenCalledTimes(1);
    view.dispose();
  });

  test('dedicates a one-output rail to media without a selector or visible caption', () => {
    const onStateChange = jest.fn();
    const state = createDefaultCubeSurfaceState();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1)],
      state,
      onStateChange,
    });

    view.renderPreview({
      outputs: [
        {
          id: 'output.image',
          label: '<b>friendly image</b>',
          items: [{ key: 'output', url: '/output.png', label: 'Cube output' }],
        },
      ],
    });

    const rail = view.element.querySelector('[data-cube-preview-rail]');
    expect(rail?.querySelectorAll('img')).toHaveLength(1);
    expect(rail?.querySelectorAll('[data-cube-preview-output]')).toHaveLength(1);
    expect(rail?.querySelector('select')).toBeNull();
    expect(rail?.querySelector('figcaption')).toBeNull();
    expect(rail?.querySelector('[data-cube-preview-output-label]')?.textContent).toBe(
      'output.image',
    );
    expect([...(rail?.querySelectorAll('img') ?? [])].map((image) => image.loading)).toEqual([
      'eager',
    ]);
    expect(rail?.textContent).toBe('output.image');
    expect(rail?.querySelector('script')).toBeNull();
    expect(state.preview.selectedOutput).toBeNull();
    expect(onStateChange).not.toHaveBeenCalled();
  });

  test('subdivides multiple Cube outputs into equal horizontal sections', () => {
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1)],
      state: createDefaultCubeSurfaceState(),
      onStateChange: jest.fn(),
    });

    view.renderPreview({
      outputs: [
        {
          id: 'output.image',
          label: 'friendly image',
          items: [{ key: 'left-image', url: '/left.png', label: 'Left output' }],
        },
        {
          id: 'output.mask',
          label: 'friendly mask',
          items: [{ key: 'right-image', url: '/right.png', label: 'Right output' }],
        },
      ],
    });

    const rail = view.element.querySelector<HTMLElement>('[data-cube-preview-rail]');
    const outputGrid = rail?.querySelector<HTMLElement>('[data-cube-preview-outputs]');
    expect(rail?.querySelectorAll('[data-cube-preview-output]')).toHaveLength(2);
    expect(outputGrid?.style.gridTemplateRows).toBe('repeat(2, minmax(0, 1fr))');
    expect(outputGrid?.style.gridTemplateColumns).toBe('');
    expect(rail?.querySelector('.sugarcubes-cube-face__preview-internal')).toBeNull();
    expect(rail?.querySelector('select')).toBeNull();
    expect(rail?.querySelector('figcaption')).toBeNull();
    expect(
      [...(rail?.querySelectorAll('[data-cube-preview-output-title]') ?? [])].map(
        (title) => title.textContent,
      ),
    ).toEqual(['output.image', 'output.mask']);
    expect(rail?.textContent).toBe('output.imageoutput.mask');
  });

  test('offers native image actions from the output rail', () => {
    const previewActions: CubePreviewActions = {
      openContextMenu: jest.fn(),
      download: jest.fn(),
    };
    const item = { key: 'proof', url: '/proof.png', label: 'image' };
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1)],
      state: createDefaultCubeSurfaceState(),
      previewActions,
      onStateChange: jest.fn(),
    });
    view.renderPreview({ outputs: [{ id: 'image', label: 'image', items: [item] }] });
    const figure = view.element.querySelector<HTMLElement>('[data-cube-preview-item="proof"]');
    const download = view.element.querySelector<HTMLButtonElement>('[data-cube-preview-download]');
    const contextMenu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });

    figure?.dispatchEvent(contextMenu);
    download?.click();

    expect(previewActions.openContextMenu).toHaveBeenCalledWith(item, contextMenu);
    expect(previewActions.download).toHaveBeenCalledWith(item);
    expect(download?.getAttribute('aria-label')).toBe('Download image');
    expect(download?.querySelector('.pi.pi-download')).not.toBeNull();
    view.dispose();
  });

  test('resizes and persists the output rail through its dedicated divider', () => {
    const state = createDefaultCubeSurfaceState();
    state.minimumColumnWidth = 240;
    state.preview.width = 320;
    const onStateChange = jest.fn();
    const view = new CubeSurfaceView({
      document,
      renderer: createRenderer(),
      identity: cubeIdentity('Cube'),
      nodes: [createNode(1)],
      state,
      getScale: () => 0.5,
      onStateChange,
    });
    view.layout(900);
    const divider = view.element.querySelector<HTMLButtonElement>('[data-cube-preview-divider]');
    if (!divider) throw new Error('Missing output preview divider.');

    divider.dispatchEvent(pointerEvent('pointerdown', 500, 7));
    window.dispatchEvent(pointerEvent('pointermove', 460, 7));
    window.dispatchEvent(pointerEvent('pointerup', 460, 7));

    expect(state.preview.width).toBe(400);
    expect(view.element.querySelector<HTMLElement>('[data-cube-preview-rail]')?.style.width).toBe(
      '400px',
    );
    expect(onStateChange).toHaveBeenCalledTimes(1);
    expect(onStateChange).toHaveBeenCalledWith(state);

    const masonry = view.element.querySelector<HTMLElement>('[data-cube-masonry]');
    const dividerAdjustedMasonryWidth = Number.parseFloat(masonry?.style.width ?? '');
    view.layout(1_100);
    expect(Number.parseFloat(masonry?.style.width ?? '')).toBe(dividerAdjustedMasonryWidth);
    expect(view.element.querySelector<HTMLElement>('[data-cube-preview-rail]')?.style.width).toBe(
      '600px',
    );

    view.layout(250);
    expect(divider.hidden).toBe(true);
    expect(
      view.element.querySelector<HTMLElement>('[data-cube-content]')?.dataset.previewLayout,
    ).toBe('stacked');
    view.dispose();
  });
});

/** Build one pointer-shaped event for divider interaction tests. */
function pointerEvent(type: string, clientX: number, pointerId: number): PointerEvent {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    button: { value: 0 },
    clientX: { value: clientX },
    pointerId: { value: pointerId },
  });
  return event as PointerEvent;
}

function createNode(index: number): ComfyNode {
  return {
    id: String(index),
    type: `Node${index}`,
    size: [100, 100],
    widgets: [{ name: 'value' }],
  };
}

function createRenderer(): NativeNodeCardRenderer {
  return {
    mount: jest.fn(
      (target: HTMLElement, node: ComfyNode, options: NativeNodeCardMountOptions = {}) => {
        target.textContent = node.type ?? '';
        if (options.headerAccessory) {
          target.append(options.headerAccessory);
        }
        return {
          refresh: jest.fn(),
          unmount: jest.fn(),
        };
      },
    ),
    dispose: jest.fn(),
  };
}

/** Build the fully resolved header model supplied by the Cube presenter. */
function cubeIdentity(instanceTitle: string): CubeIdentityPresentation {
  return {
    instanceTitle,
    definitionTitle: 'SDXL/Text to Image',
    versionText: 'version 2.0.0',
    definitionLine: 'SDXL/Text to Image version 2.0.0',
    awaitingFirstSave: false,
    sourceLine: 'from Base-Cubes by Artificial-Sweetener',
    icon: {
      kind: 'asset',
      url: '/cube-icon.png',
      mediaType: 'image/png',
      initials: 'TI',
      fallback: {
        fontFamily: 'Segoe UI',
        fontWeight: 700,
        inset: 2,
        renderSize: 96,
      },
    },
  };
}

/** Snapshot only geometry that transient header UI must never own. */
function readFaceGeometry(
  content: HTMLElement,
  card: HTMLElement,
  minimumHeights: readonly number[],
): object {
  return {
    contentMinHeight: content.style.minHeight,
    contentScrollLeft: content.scrollLeft,
    contentScrollTop: content.scrollTop,
    cardLeft: card.style.left,
    cardTop: card.style.top,
    cardWidth: card.style.width,
    minimumHeight: minimumHeights.at(-1),
  };
}
