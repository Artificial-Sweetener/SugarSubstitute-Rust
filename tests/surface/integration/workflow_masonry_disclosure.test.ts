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
/** Prove neighboring masonry cards follow one destination during native disclosure. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryView } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryView.js';
import {
  publishDisclosureHeight,
  measureDisclosureCardHeight,
} from '../../../frontend/comfyui/ui/surface/motion/DisclosureLayout.js';
import { SurfaceMotion } from '../../../frontend/comfyui/ui/surface/motion/SurfaceMotion.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type {
  NativeNodeCardMountOptions,
  NativeNodeCardRenderer,
} from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { NativeProjectedNodeCollapse } from '../../../frontend/comfyui/ui/surface/NativeProjectedNodeCollapse.js';
import { CardDisclosureMotion } from '../../../frontend/comfyui/ui/surface/motion/CardDisclosureMotion.js';
import { DISCLOSURE_LAYOUT_EVENT } from '../../../frontend/comfyui/ui/surface/motion/DisclosureLayout.js';
import { fluentInteractionCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentInteractionStyles.js';
import { NativeProjectedAdvancedInputs } from '../../../frontend/comfyui/ui/surface/NativeProjectedAdvancedInputs.js';
import { NativeAdvancedDisclosure } from '../../../frontend/comfyui/ui/surface/appearance/NativeAdvancedDisclosure.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
  jest.useRealTimers();
});

test('packs simultaneous disclosure destinations once per frame without rebuilding cards', async () => {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const view = new WorkflowMasonryView({
    document,
    sections: projectWorkflowMasonry(
      { _nodes: [{ id: 1, type: 'KSampler', widgets: [{ name: 'steps', value: 20 }] }] },
      [],
    ),
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  const mount = jest.fn<NativeNodeCardRenderer['mount']>(() => ({ refresh() {}, unmount() {} }));
  await view.mountCards({ mount, dispose() {} });
  view.element.removeAttribute('data-sc-masonry-preparing');
  for (const callback of frames.splice(0)) callback(0);
  const layout = jest.spyOn(view, 'layout');
  const cell = [...view.cards().values()][0]!;
  const first = document.createElement('div');
  const second = document.createElement('div');
  cell.append(first, second);
  publishDisclosureHeight(first, 80);
  publishDisclosureHeight(second, 100);
  expect(layout).not.toHaveBeenCalled();
  for (const callback of frames.splice(0)) callback(16);
  expect(layout).toHaveBeenCalledTimes(1);
  expect(mount).toHaveBeenCalledTimes(1);
  view.dispose();
});

test('search masks pack visible cards and hide empty headers without changing native mounts or policy', async () => {
  const nodes: ComfyNode[] = [
    { id: 1, type: 'KSampler', widgets: [{ name: 'steps', value: 20 }] },
    { id: 2, type: 'KSampler', widgets: [{ name: 'steps', value: 30 }] },
  ];
  const sections = projectWorkflowMasonry({ _nodes: nodes }, []);
  const mount = jest.fn<NativeNodeCardRenderer['mount']>(() => ({ refresh() {}, unmount() {} }));
  const view = new WorkflowMasonryView({
    document,
    sections,
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  await view.mountCards({ mount, dispose() {} });
  const [first, second] = [...view.cards()];
  if (!first || !second) throw new Error('Native card fixture missing');
  view.setSearchCards(new Set([second[0]]));
  expect(first[1].hasAttribute('data-sc-search-card-hidden')).toBe(true);
  expect(second[1].hasAttribute('data-sc-search-card-hidden')).toBe(false);
  view.setSearchCards(new Set());
  expect(
    view.element.querySelector('header[data-masonry-section][data-sc-search-card-hidden]'),
  ).not.toBeNull();
  view.setSearchCards(null);
  expect(view.cards().get(first[0])).toBe(first[1]);
  expect(first[1].hasAttribute('data-sc-search-card-hidden')).toBe(false);
  expect(sections.flatMap((section) => section.cards).every((card) => card.visible)).toBe(true);
  expect(mount).toHaveBeenCalledTimes(2);
  view.dispose();
});

test('masonry suppresses disclosure only for semantic prompt cards', async () => {
  const nodes: ComfyNode[] = [
    {
      id: 'positive',
      title: 'Positive prompt',
      type: 'StringConstant',
      widgets: [{ name: 'text', type: 'customtext', value: 'prompt' }],
    },
    { id: 'sampler', type: 'KSampler', widgets: [{ name: 'steps', value: 20 }] },
  ];
  const mounts = new Map<ComfyNode, NativeNodeCardMountOptions>();
  const view = new WorkflowMasonryView({
    document,
    sections: projectWorkflowMasonry({ _nodes: nodes }, []),
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  const renderer: NativeNodeCardRenderer = {
    mount(_target, node, options = {}) {
      mounts.set(node, options);
      return { refresh() {}, unmount() {} };
    },
    dispose() {},
  };
  await view.mountCards(renderer);
  expect(mounts.get(nodes[0]!)).toMatchObject({ allowCollapse: false });
  expect(mounts.get(nodes[1]!)).toMatchObject({ allowCollapse: true });
  nodes[0]!.title = 'Notes';
  await view.refresh(projectWorkflowMasonry({ _nodes: nodes }, []), renderer);
  expect(mounts.get(nodes[0]!)).toMatchObject({ allowCollapse: true });
  view.dispose();
});

/** Model a native prompt whose Fluent textarea sizing follows reactive expansion. */
function promptDisclosureFixture() {
  jest.useFakeTimers({ doNotFake: ['queueMicrotask'] });
  const target = document.createElement('div');
  target.style.height = '50px';
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.scStyleNode = '';
  root.style.height = '50px';
  const header = document.createElement('div');
  header.dataset.testid = 'node-header-prompt';
  header.style.height = '49px';
  const button = document.createElement('button');
  button.dataset.testid = 'node-collapse-button';
  header.append(button);
  root.append(header);
  target.append(root);
  const cubeRoot = document.createElement('div');
  cubeRoot.className = 'lg-node';
  cubeRoot.dataset.scStyleNode = '';
  cubeRoot.style.height = '1000px';
  cubeRoot.append(target);
  document.body.append(cubeRoot);
  const flags = { collapsed: true };
  const node: ComfyNode = {
    flags,
    collapse: () => {
      flags.collapsed = !flags.collapsed;
    },
  };
  const effects: { frames: Keyframe[]; finish(): void }[] = [];
  Object.defineProperty(target, 'animate', {
    value: (frames: Keyframe[]) => {
      let finish = (): void => undefined;
      const finished = new Promise<void>((resolve) => {
        finish = resolve;
      });
      effects.push({ frames, finish });
      return { finished, cancel: jest.fn() };
    },
  });
  const motion = new CardDisclosureMotion(target);
  const collapse = new NativeProjectedNodeCollapse(
    target,
    node,
    () => {
      root.style.height = flags.collapsed ? '50px' : '130px';
      window.requestAnimationFrame(() => {
        root.style.height = flags.collapsed ? '50px' : '100px';
      });
    },
    motion,
  );
  return { target, root, button, flags, effects, collapse, motion, node };
}

test('expanding a native prompt publishes its settled height without an oversized neighbor destination', async () => {
  const { target, root, button, effects, collapse, motion } = promptDisclosureFixture();
  const destinations: (string | undefined)[] = [];
  target.addEventListener(DISCLOSURE_LAYOUT_EVENT, () =>
    destinations.push(target.dataset.scDisclosureHeight),
  );
  button.click();
  expect(root.style.height).toBe('130px');
  expect(target.style.height).toBe('50px');
  expect(effects).toHaveLength(0);
  await jest.runAllTimersAsync();
  expect(root.style.height).toBe('100px');
  expect(effects[0]?.frames).toEqual([{ height: '50px' }, { height: '100px' }]);
  expect(destinations).toEqual(['100']);
  effects[0]?.finish();
  await Promise.resolve();
  expect(target.style.height).toBe('50px');
  expect(destinations).toEqual(['100', undefined]);
  collapse.dispose();
  motion.dispose();
});

test('a registered advanced footer never constrains whole-card expansion to its collapsed height', async () => {
  const { target, root, button, effects, collapse, motion, node } = promptDisclosureFixture();
  target.classList.add('sugarcubes-native-node-card');
  node.widgets = [{ name: 'tile_size', type: 'number', options: { advanced: true } }];
  const fields = document.createElement('div');
  const advancedButton = document.createElement('button');
  advancedButton.dataset.testid = 'advanced-inputs-button';
  root.append(fields, advancedButton);
  const input = new NativeProjectedAdvancedInputs(target, node, () => undefined, motion);
  const presentation = new NativeAdvancedDisclosure();
  presentation.reconcile({
    root,
    body: null,
    header: null,
    fields: [],
    numericFields: [],
    seedFields: [],
    labelElements: [],
    collapsed: true,
    canCollapse: true,
    advanced: { fields, button: advancedButton, expanded: false },
  });
  const styles = document.createElement('style');
  styles.textContent = fluentInteractionCss();
  document.body.append(styles);
  button.click();
  const fillRule = [...(styles.sheet?.cssRules ?? [])].find(
    (rule): rule is CSSStyleRule =>
      rule instanceof CSSStyleRule && rule.style.getPropertyValue('height') === '100%',
  );
  if (!fillRule) throw new Error('Advanced disclosure fill rule missing');
  expect(root.matches(fillRule.selectorText)).toBe(false);
  await jest.runAllTimersAsync();
  expect(effects[0]?.frames).toEqual([{ height: '50px' }, { height: '100px' }]);
  effects[0]?.finish();
  await Promise.resolve();
  expect(target.hasAttribute('data-sc-disclosing')).toBe(false);
  input.dispose();
  presentation.clear();
  collapse.dispose();
  motion.dispose();
});

test('reversing or disposing a pending expansion prevents a stale oversized animation', async () => {
  const first = promptDisclosureFixture();
  first.button.click();
  first.button.click();
  await jest.runAllTimersAsync();
  expect(first.effects).toHaveLength(1);
  expect(first.effects[0]?.frames[1]).toEqual({ height: '49px' });
  first.effects[0]?.finish();
  await jest.runAllTimersAsync();
  expect(first.flags.collapsed).toBe(true);
  first.collapse.dispose();
  first.motion.dispose();
  const second = promptDisclosureFixture();
  second.button.click();
  second.collapse.dispose();
  second.motion.dispose();
  await jest.runAllTimersAsync();
  expect(second.effects).toHaveLength(0);
  expect(second.flags.collapsed).toBe(false);
  expect(second.target.hasAttribute('data-sc-disclosing')).toBe(false);
});

test('packs the settled height once and never sends neighbors through intermediate columns', async () => {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const nodes: ComfyNode[] = [1, 2, 3].map((id) => ({ id, type: 'Example', widgets: [] }));
  const sections: WorkflowMasonrySection[] = [
    {
      id: 'loose',
      label: 'Workflow',
      cube: null,
      target: { _nodes: nodes },
      cards: nodes.map((node) => ({
        id: String(node.id),
        node,
        columnSpan: 1,
        label: 'Node',
        visible: true,
        enabled: true,
        showActivationControl: false,
      })),
    },
  ];
  const view = new WorkflowMasonryView({
    document,
    sections,
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  const content = view.element.firstElementChild;
  if (!content) throw new Error('Masonry content missing');
  Object.defineProperty(content, 'clientWidth', { value: 824 });
  const heights = new Map([
    [1, 200],
    [2, 300],
    [3, 100],
  ]);
  const place = jest.spyOn(SurfaceMotion.prototype, 'place');
  await view.mountCards({
    mount(target, node) {
      const cell = target.parentElement;
      if (!cell) throw new Error('Native mount missing cell');
      cell.style.height = `${String(heights.get(Number(node.id)))}px`;
      cell.style.boxSizing = 'border-box';
      return { refresh() {}, unmount() {} };
    },
    dispose() {},
  });
  view.setTransitioning(true);
  view.element.removeAttribute('data-sc-masonry-preparing');
  view.setTransitioning(false);
  const first = content.querySelector<HTMLElement>('[data-masonry-card="1"]');
  const next = content.querySelector<HTMLElement>('[data-masonry-card="3"]');
  if (!first || !next) throw new Error('Masonry fixture cards missing');
  expect(next.style.transform).toBe('translate(0px, 256px)');
  const fields = document.createElement('div');
  fields.style.height = '100px';
  first.append(fields);
  publishDisclosureHeight(fields, 300);
  for (const callback of frames.splice(0)) callback(16);
  expect(next.style.transform).toBe('translate(418px, 356px)');
  const destination = next.style.transform;
  place.mockClear();
  for (const current of [140.125, 220.25, 275.875, 300]) {
    fields.style.height = `${String(current)}px`;
    first.style.height = `${String(current + 100)}px`;
    expect(measureDisclosureCardHeight(first)).toBe(400);
    view.layout();
    expect(next.style.transform).toBe(destination);
  }
  expect(
    place.mock.calls
      .filter(([placements]) => placements.some(({ element }) => element === next))
      .every(([, animate]) => animate !== false),
  ).toBe(true);
  publishDisclosureHeight(fields, null);
  expect(next.style.transform).toBe(destination);
  view.dispose();
  publishDisclosureHeight(fields, 900);
  expect(view.element.isConnected).toBe(false);
});

test('uses the outer disclosure destination when an inner field is also disclosing', () => {
  const card = document.createElement('article');
  card.style.height = '200px';
  card.style.boxSizing = 'border-box';
  const mount = document.createElement('div');
  mount.style.height = '200px';
  const fields = document.createElement('div');
  fields.style.height = '100px';
  mount.append(fields);
  card.append(mount);
  document.body.append(card);
  publishDisclosureHeight(fields, 300);
  expect(measureDisclosureCardHeight(card)).toBe(400);
  publishDisclosureHeight(mount, 40);
  expect(measureDisclosureCardHeight(card)).toBe(40);
  publishDisclosureHeight(mount, null);
  publishDisclosureHeight(fields, null);
  expect(measureDisclosureCardHeight(card)).toBe(200);
});
