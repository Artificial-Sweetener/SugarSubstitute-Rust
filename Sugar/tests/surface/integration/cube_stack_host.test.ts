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
/** Prove persisted rail lifetimes, appearance, navigation and canonical mutation wiring. */
import { beforeEach, afterEach, expect, jest, test } from '@jest/globals';
import { nextTick } from 'vue';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import { DIALOG_OVERLAY_Z_INDEX } from '../../../frontend/comfyui/ui/core/OverlayStacking.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';
import {
  CubeStackHost,
  type CubeStackSurface,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackHost.js';
import {
  CUBE_STACK_VISIBLE_SETTING,
  CUBE_STACK_COMPACT_SETTING,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackPreferences.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { projectCubeStackRows } from '../../../frontend/comfyui/ui/surface/stack/CubeStackProjection.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';
import { CubeStackDrawerMotion } from '../../../frontend/comfyui/ui/surface/stack/CubeStackDrawerMotion.js';
import { SubstituteFrameMode } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameMode.js';
import { publishPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';
import { pluginFrameCss } from '../../../frontend/comfyui/ui/surface/frame/PluginFrameStyles.js';
import { COMPACT_MASONRY_CHANGE } from '../../../frontend/comfyui/ui/surface/masonry/CompactMasonryPreference.js';
import type { InsetMotion } from '../../../frontend/comfyui/ui/surface/appearance/TransformMotion.js';

test('Plugin Mode compacts the existing rail and removes extra space before the cards', async () => {
  const frame = new SubstituteFrameMode();
  frame.setEnabled(true);
  const fixture = setup(true, true, frame);
  fixture.surface.element.className = 'sugarcubes-workflow-masonry';
  ensureWorkflowMasonryStyles(document);
  const style = document.createElement('style');
  style.textContent = pluginFrameCss;
  document.head.append(style);
  fixture.host.setup();
  fixture.host.ready(fixture.surface, false);
  await nextTick();
  const rail = document.querySelector('.sc-cube-stack-host');
  publishPluginMode(document, true);
  await nextTick();
  expect(document.querySelector('.sc-cube-stack-host')).toBe(rail);
  expect(rail?.querySelector('[data-compact]')).not.toBeNull();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(46, 'transform');
  expect(
    getComputedStyle(fixture.surface.element).getPropertyValue('--sc-masonry-inline-padding'),
  ).toBe('0px');
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(false);
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(46, 'transform');
  publishPluginMode(document, false);
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(224, 'transform');
  expect(document.querySelector('.sc-cube-stack-host')).toBe(rail);
});

test('frame mode changes allocation without replacing the existing rail or accent indicator', async () => {
  const frame = new SubstituteFrameMode();
  const fixture = setup(true, true, frame);
  fixture.host.setup();
  fixture.host.ready(fixture.surface, false);
  await nextTick();
  const rail = document.querySelector('.sc-cube-stack-host');
  const indicator = rail?.querySelector('.sc-cube-stack__selection');
  frame.setEnabled(true);
  await nextTick();
  // Substitute moves the cards on the stack's density frame; the browser keeps the surface curve.
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(224, 'transform');
  expect(document.querySelector('.sc-cube-stack-host')).toBe(rail);
  expect(rail?.querySelector('.sc-cube-stack__selection')).toBe(indicator);
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(true);
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(58, 'transform');
  frame.setEnabled(false);
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(82, 'surface');
});

test('compact Cube stack keeps masonry spacing while Compact masonry packs the rail flush', async () => {
  const fixture = setup();
  fixture.surface.element.className = 'sugarcubes-workflow-masonry';
  ensureWorkflowMasonryStyles(document);
  fixture.host.setup();
  fixture.host.ready(fixture.surface, false);
  await nextTick();
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(true);
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(82, 'surface');
  const gap = (): string =>
    getComputedStyle(fixture.surface.element).getPropertyValue('--sugarcubes-masonry-gap');
  expect(gap()).not.toBe('0');
  document.documentElement.setAttribute('data-sc-compact-masonry', '');
  document.dispatchEvent(new Event(COMPACT_MASONRY_CHANGE));
  await nextTick();
  try {
    expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(46, 'surface');
    expect(gap()).toBe('0');
    expect(document.querySelector('.sc-cube-stack')?.hasAttribute('data-compact')).toBe(true);
    fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(false);
    await nextTick();
    expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(224, 'surface');
  } finally {
    document.documentElement.removeAttribute('data-sc-compact-masonry');
  }
});

const owned: CubeStackHost[] = [];
test.each([0, 1, 2])(
  'routes non-overflowing stack wheel units %s into its paired masonry',
  async (deltaMode) => {
    const fixture = setup();
    fixture.host.setup();
    fixture.host.ready(fixture.surface);
    await nextTick();
    const rail = document.querySelector<HTMLElement>('.sc-cube-stack-host')!;
    Object.defineProperties(rail, { clientHeight: { value: 600 }, scrollHeight: { value: 600 } });
    Object.defineProperty(fixture.surface.element, 'clientHeight', { value: 500 });
    const event = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaY: 2,
      deltaMode,
    });
    rail.querySelector('button')!.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(fixture.surface.element.scrollTop).toBe(
      deltaMode === 1 ? 32 : deltaMode === 2 ? 1000 : 2,
    );
    fixture.host.release();
    rail.dispatchEvent(new WheelEvent('wheel', { cancelable: true, deltaY: 50 }));
    expect(fixture.surface.element.scrollTop).toBe(
      deltaMode === 1 ? 32 : deltaMode === 2 ? 1000 : 2,
    );
  },
);

test('preserves scrolling stacks, nested controls, horizontal gestures and zoom', async () => {
  const fixture = setup();
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  await nextTick();
  const rail = document.querySelector<HTMLElement>('.sc-cube-stack-host')!;
  Object.defineProperties(rail, {
    clientHeight: { value: 300 },
    scrollHeight: { value: 600, configurable: true },
  });
  const dispatch = (target: HTMLElement, options: WheelEventInit): void => {
    const event = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaY: 50,
      ...options,
    });
    target.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
    expect(fixture.surface.element.scrollTop).toBe(0);
  };
  dispatch(rail, {});
  Object.defineProperty(rail, 'scrollHeight', { value: 300 });
  for (const options of [
    { ctrlKey: true },
    { metaKey: true },
    { shiftKey: true },
    { deltaY: 0, deltaX: 50 },
  ])
    dispatch(rail, options);
  const nested = document.createElement('div');
  nested.style.overflowY = 'auto';
  Object.defineProperties(nested, { clientHeight: { value: 50 }, scrollHeight: { value: 100 } });
  rail.append(nested);
  dispatch(nested, {});
});

test('restores an already-selected workflow rail without allocating entrance animations', () => {
  const fixture = setup();
  fixture.host.setup();
  const animate = jest.spyOn(CubeStackDrawerMotion.prototype, 'setVisible');
  fixture.host.ready(fixture.surface, false);
  expect(animate).not.toHaveBeenCalled();
  expect(document.querySelector<HTMLElement>('.sc-cube-stack-host')?.style.opacity).toBe('1');
  expect(document.querySelector<HTMLElement>('.sc-cube-stack-host')?.inert).not.toBe(true);
});
beforeEach(() => {
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => null);
});
afterEach(() => {
  for (const host of owned.splice(0)) host.dispose();
  publishPluginMode(document, false);
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

test.each([
  ['Anima/Text to Image', 'Anima', 'Text to Image'],
  ['anima/Contextual Upscale 2', 'Anima', 'Contextual Upscale 2'],
  ['Hero Background', 'Anima', 'Hero Background'],
  ['Other/Text to Image', 'Anima', 'Other/Text to Image'],
  ['Anima/', 'Anima', 'Anima/'],
])('separates the canonical model pill from stack title %s', async (title, model, label) => {
  const fixture = setup();
  const cube = fixture.surface.sections[0]!.cube;
  cube.title = title;
  cube.properties.sugarcubes_cube = {
    instance_id: 'first',
    cube_id: 'Artificial-Sweetener/Base-Cubes/Anima/Text to Image.cube',
    cube_version: '3.1.0',
    target_model: model,
  };
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  await nextTick();
  const button = document.querySelector('.sc-cube-stack-row__activate');
  expect(button?.querySelector('.sc-cube-stack-row__label')?.textContent).toBe(label);
  expect(button?.querySelector('.sc-cube-stack-row__secondary')?.textContent).toBe(
    'v3.1.0 · base-cubes',
  );
  expect(button?.querySelector('.sc-cube-stack-row__model')?.textContent).toBe(model);
});

test.each([
  ['Artificial-Sweetener/Base-Cubes/Anima/Cube.cube', '3.1.0', 'v3.1.0 · base-cubes'],
  ['Org/My Pack/Cube.cube', 'v2.0.0', 'v2.0.0 · my-pack'],
  ['Org\\base_cubes\\Cube.cube', '1.0.0', 'v1.0.0 · base-cubes'],
  ['local/personal/Anima/Cube.cube', '1.0.0', 'v1.0.0 · personal'],
  ['Org/Base-Cubes/Cube.cube', '', 'base-cubes'],
  ['Cube.cube', '1.0.0', 'v1.0.0'],
  ['', '', ''],
])('formats compact stack metadata from %s', (cubeId, version, secondary) => {
  const cube = cubeNode('first');
  cube.properties.sugarcubes_cube = {
    instance_id: 'first',
    cube_id: cubeId,
    cube_version: version,
    target_model: 'Anima',
  };
  const rows = projectCubeStackRows(
    [{ id: 'cube:first', label: 'first', cube, target: cube, cards: [] }],
    null,
    'first',
    () => null,
    true,
  );
  expect(rows[0]?.secondary).toBe(secondary);
  expect(rows[0]?.targetModel).toBe('Anima');
});

test('places the actual rail above masonry backdrop and below modal interactions', () => {
  const fixture = setup();
  const backdrop = document.createElement('div');
  backdrop.className = 'sugarcubes-workflow-masonry__backdrop';
  fixture.surface.element.parentElement!.style.pointerEvents = 'none';
  fixture.surface.element.before(backdrop);
  fixture.surface.element.className = 'sugarcubes-workflow-masonry';
  ensureWorkflowMasonryStyles(document);
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  const rail = document.querySelector<HTMLElement>('.sc-cube-stack-host');
  if (!rail) throw new Error('Cube rail was not mounted.');
  expect(getComputedStyle(rail).pointerEvents).toBe('auto');
  const toolbar = document.createElement('div');
  toolbar.className = 'sc-overrides-toolbar';
  document.body.append(toolbar);
  ensureRelationStyles(document);
  // Native chrome is outside the graph panel; reserve only the workspace edge margin.
  expect(getComputedStyle(rail).top).toBe('12px');
  expect(document.getElementById('sugarcubes-workflow-masonry-styles')?.textContent).toContain(
    'padding:12px var(--sc-masonry-inline-padding,20px)',
  );
  const railLayer = Number(getComputedStyle(rail).zIndex);
  expect(railLayer).toBeGreaterThan(Number(getComputedStyle(backdrop).zIndex));
  expect(railLayer).toBeGreaterThan(Number(getComputedStyle(fixture.surface.element).zIndex));
  expect(railLayer).toBeLessThan(DIALOG_OVERLAY_Z_INDEX);
});

test.each([
  ['Anima/Contextual Upscale 2', 'Contextual Upscale 2'],
  ['Final polish', 'Final polish'],
  ['Flux/Final polish 42', 'Flux/Final polish 42'],
])('projects the shared instance title and model for %s', (title, label) => {
  const cube = cubeNode('first');
  cube.title = title;
  cube.properties.sugarcubes_cube = {
    instance_id: 'first',
    target_model: 'Anima',
    default_alias: 'Anima/Contextual Upscale',
  };
  const rows = projectCubeStackRows(
    [{ id: 'cube:first', label: title, cube, target: cube, cards: [] }],
    null,
    'first',
    () => null,
    true,
  );
  expect(rows[0]).toMatchObject({ label, targetModel: 'Anima' });
  expect(cube.title).toBe(title);
});

test('dims a bypassed thumbnail while preserving the model capsule opacity and geometry', async () => {
  const f = setup();
  f.surface.sections[0]!.cube!.mode = 4;
  f.host.setup();
  f.host.ready(f.surface);
  await nextTick();
  const row = document.querySelector('.sc-cube-stack-row[data-bypassed]');
  const asset = row?.querySelector('.sc-cube-stack-row__asset');
  const pill = row?.querySelector('.sc-cube-stack-row__model');
  if (!asset || !pill) throw new Error('Bypassed thumbnail and model capsule were not rendered.');
  expect(getComputedStyle(asset).opacity).toBe('0.42');
  expect(getComputedStyle(asset.firstElementChild!).flexBasis).toBe('100%');
  expect(getComputedStyle(pill).opacity).not.toBe('0.42');
  const style = getComputedStyle(pill);
  expect(style.height).toBe('10px');
  expect(style.right).toBe('-3px');
  expect(style.bottom).toBe('-2px');
});

test('retains one mounted rail, follows official appearance and recovers a hidden persisted rail', async () => {
  const fixture = setup();
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  await nextTick();
  const rail = document.querySelector('.sc-cube-stack');
  const button = document.querySelector('.sc-cube-stack-row__activate');
  expect(button?.querySelector('.sc-cube-stack-row__model')?.textContent).toBe('SDXL');
  expect(document.querySelector('.sc-cube-stack-row__secondary')?.textContent).toContain('v1.0.0');
  fixture.host.ready(fixture.surface);
  fixture.appearance.update('fluent', false);
  await nextTick();
  expect(document.querySelector('.sc-cube-stack')).toBe(rail);
  expect(document.querySelector('.sc-cube-stack-row__activate')).toBe(button);
  expect(rail?.getAttribute('data-appearance')).toBe('fluent');
  expect(rail?.parentElement?.dataset.colorScheme).toBe('light');
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(248, 'surface');
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(true);
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(82, 'surface');
  fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
  expect(document.querySelector('.sc-cube-stack')).toBeNull();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(0, 'surface');
  fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(true);
  await nextTick();
  expect(document.querySelector('.sc-cube-stack')).not.toBeNull();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(82, 'surface');
});

test('routes navigation through search and waits for accepted masonry state after keyboard reorder', async () => {
  const fixture = setup();
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  const button = document.querySelector<HTMLButtonElement>('.sc-cube-stack-row__activate');
  button?.click();
  expect(fixture.revealSearchResults).toHaveBeenCalledTimes(1);
  expect(fixture.surface.scrollToSection).toHaveBeenCalledWith('cube:first');
  button?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowDown', altKey: true, bubbles: true }),
  );
  for (let tick = 0; tick < 8; tick += 1) await nextTick();
  expect(fixture.move).toHaveBeenCalledWith('first', 1);
  expect(fixture.refreshSurface).toHaveBeenCalledTimes(1);
  expect(fixture.surface.previewCubeOrder).toHaveBeenLastCalledWith(null);
});

test('retains the same Vue controls until drawer exit finishes and survives a quick reopen', async () => {
  const completions: (() => void)[] = [];
  Object.defineProperty(HTMLElement.prototype, 'animate', {
    configurable: true,
    value: () => ({
      currentTime: 0,
      finished: new Promise<void>((resolve) => completions.push(resolve)),
      pause() {},
      play() {},
      updatePlaybackRate() {},
      cancel() {},
    }),
  });
  try {
    const fixture = setup();
    fixture.host.setup();
    fixture.host.ready(fixture.surface);
    const row = document.querySelector('.sc-cube-stack-row__activate');
    fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
    expect(document.querySelector('.sc-cube-stack-row__activate')).toBe(row);
    expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(0, 'surface');
    fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(true);
    for (const finish of completions) finish();
    for (let tick = 0; tick < 8; tick += 1) await nextTick();
    expect(document.querySelector('.sc-cube-stack-row__activate')).toBe(row);
    expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(248, 'surface');
  } finally {
    Reflect.deleteProperty(HTMLElement.prototype, 'animate');
  }
});

test('does no graph discovery for unrelated popup activity and releases the old surface before renderer teardown', async () => {
  const fixture = setup();
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  const refreshes = fixture.surface.setStackWidth.mock.calls.length;
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  popup.textContent = 'Unrelated choices';
  await nextTick();
  expect(fixture.surface.setStackWidth).toHaveBeenCalledTimes(refreshes);
  expect(fixture.refreshSurface).not.toHaveBeenCalled();
  fixture.host.release();
  expect(fixture.listeners.size).toBe(0);
  expect(document.querySelector('.sc-cube-stack-host')).toBeNull();
  fixture.appearance.update('fluent', true);
  fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
  expect(document.querySelector('.sc-cube-stack-host')).toBeNull();
  fixture.host.ready(fixture.surface);
  expect(document.querySelector('.sc-cube-stack-host')).toBeNull();
});

test('holds clicked selection during programmatic scroll and resumes whole-cube tracking on input', async () => {
  const fixture = setup();
  const element = fixture.surface.element;
  Object.defineProperty(element, 'clientHeight', { value: 500 });
  for (const [id, y] of [
    ['first', 0],
    ['second', 400],
  ] as const) {
    const header = document.createElement('header');
    header.dataset.masonrySection = `cube:${id}`;
    header.dataset.masonryY = String(y);
    header.dataset.masonryHeight = '376';
    element.append(header);
  }
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  frames.shift()!(0);
  await nextTick();
  expect(
    document.querySelector('.sc-cube-stack-row[data-selected]')?.getAttribute('data-row-id'),
  ).toBe('first');
  document.querySelectorAll<HTMLButtonElement>('.sc-cube-stack-row__activate')[1]?.click();
  fixture.host.valuesChanged();
  element.scrollTop = 100;
  element.dispatchEvent(new Event('scroll'));
  frames.shift()!(0);
  await nextTick();
  expect(
    document.querySelector('.sc-cube-stack-row[data-selected]')?.getAttribute('data-row-id'),
  ).toBe('second');
  element.dispatchEvent(new Event('scrollend'));
  await nextTick();
  expect(
    document.querySelector('.sc-cube-stack-row[data-selected]')?.getAttribute('data-row-id'),
  ).toBe('second');
  element.dispatchEvent(new WheelEvent('wheel', { deltaY: -10 }));
  frames.shift()!(0);
  await nextTick();
  expect(
    document.querySelector('.sc-cube-stack-row[data-selected]')?.getAttribute('data-row-id'),
  ).toBe('first');
  expect(fixture.refreshSurface).not.toHaveBeenCalled();
});

test.each([true, false])(
  'uses root mutation availability while analysis refreshes (%s)',
  async (available) => {
    const fixture = setup(false, available);
    fixture.host.setup();
    fixture.host.ready(fixture.surface);
    await nextTick();
    const rows = [...document.querySelectorAll('.sc-cube-stack-row')];
    expect(rows).toHaveLength(2);
    for (const row of rows) expect(row.hasAttribute('data-reorderable')).toBe(available);
  },
);

/** Provide existing mounted topology with native-shaped Cube identity and isolated host ports. */
function setup(analysisAvailable = true, available = true, frame?: SubstituteFrameMode) {
  const settings = new Map<string, { onChange(value: unknown): void }>();
  const appearance = new AppearanceState();
  const container = document.createElement('div');
  const element = document.createElement('section');
  container.append(element);
  document.body.append(container);
  const surface = {
    element,
    sections: ['first', 'second'].map((id) => {
      const cube = cubeNode(id);
      return { id: `cube:${id}`, label: id, cube, target: cube, cards: [] };
    }),
    previewCubeOrder: jest.fn<(order: readonly string[] | null) => void>(),
    setCubeOrder: jest.fn<(order: readonly string[]) => void>(),
    setStackWidth: jest.fn<(width: number, motion: InsetMotion) => void>(),
    scrollToSection: jest.fn<(id: string) => void>(),
  } satisfies CubeStackSurface;
  const listeners = new Set<() => void>();
  const analysis: CubeGraphAnalysis = {
    semanticHash: 'a'.repeat(64),
    instances: ['first', 'second'].map((instanceId) => ({
      instanceId,
      nodeId: instanceId,
      definitionId: instanceId,
      cubeId: instanceId,
      version: '1',
      alias: instanceId,
      executionMode: 0,
    })),
    placements: ['first', 'second'].map((instanceId, index) => ({
      instanceId,
      nodeId: instanceId,
      position: [index * 344, 0],
    })),
    segments: [{ instanceIds: ['first', 'second'], reorderable: true, boundaryNodeIds: [] }],
  };
  const move = jest.fn(async (_id: string, _delta: -1 | 1) => undefined);
  const revealSearchResults = jest.fn();
  const refreshSurface = jest.fn(async () => undefined);
  const openCart = jest.fn();
  const host = new CubeStackHost({
    document,
    app: {
      ui: {
        settings: {
          addSetting(setting: { id: string; onChange(value: unknown): void }) {
            settings.set(setting.id, setting);
          },
          getSettingValue(_id: string, fallback: boolean) {
            return fallback;
          },
          async setSettingValue(id: string, value: boolean) {
            settings.get(id)?.onChange(value);
          },
        },
      },
    },
    appearance,
    ...(frame ? { frame } : {}),
    getReorder: () => ({
      analysis: analysisAvailable ? analysis : null,
      available,
      pending: false,
      move,
      reorder: async () => undefined,
      refresh: async () => analysis,
      subscribe(listener) {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    }),
    resolveSource: () => null,
    revealSearchResults,
    refreshSurface,
    openCart,
    remove: jest.fn(),
    contextMenu: jest.fn(),
    closeTransient: jest.fn(),
    reportError: jest.fn(),
  });
  owned.push(host);
  return {
    host,
    surface,
    settings,
    appearance,
    move,
    listeners,
    revealSearchResults,
    refreshSurface,
    openCart,
  };
}

/** Build a native-shaped empty Cube without substituting presentation policy or graph services. */
function cubeNode(id: string): CubeNode {
  return {
    id,
    title: id,
    pos: [0, 0],
    size: [220, 160],
    inputs: [],
    outputs: [],
    mode: 0,
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: id,
        cube_id: `local/personal/${id}`,
        cube_version: '1.0.0',
        target_model: 'SDXL',
      },
    },
    connect() {},
    serialize() {
      return {};
    },
    isSubgraphNode: () => true,
    subgraph: {
      id: `definition:${id}`,
      name: id,
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      configure() {},
      addInput() {
        return { connect() {} };
      },
      addOutput() {
        return { connect() {} };
      },
    },
  };
}

test('empty masonry reserves no rail and introduces the first cube through existing drawer motion', async () => {
  const fixture = setup();
  const first = fixture.surface.sections[0]!;
  fixture.surface.sections = [];
  fixture.host.setup();
  fixture.host.ready(fixture.surface);
  await nextTick();
  expect(document.querySelector('.sc-cube-stack-host')).toBeNull();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(0, 'surface');
  expect(fixture.host.frameControls.available).toBe(true);
  expect(fixture.host.frameControls.action).toBe('add');
  await fixture.host.frameControls.activate();
  expect(fixture.openCart).toHaveBeenCalledTimes(1);
  expect(fixture.host.frameControls.action).toBe('add');
  const present = jest.spyOn(CubeStackDrawerMotion.prototype, 'setVisible');
  fixture.surface.sections = [first];
  fixture.host.ready(fixture.surface);
  await nextTick();
  expect(document.querySelectorAll('.sc-cube-stack-row')).toHaveLength(1);
  expect(present).toHaveBeenCalledWith(true);
  expect(fixture.host.frameControls.action).toBe('collapse');
  fixture.surface.sections = [];
  fixture.host.valuesChanged();
  await nextTick();
  expect(document.querySelector('.sc-cube-stack-host')).toBeNull();
  expect(fixture.surface.setStackWidth).toHaveBeenLastCalledWith(0, 'surface');
  expect(fixture.host.frameControls.action).toBe('add');
  await fixture.host.transition(false);
  await fixture.host.frameControls.activate();
  expect(fixture.openCart).toHaveBeenCalledTimes(1);
});

test('frame compact control follows visible masonry without changing preferences while unavailable', async () => {
  const fixture = setup();
  fixture.host.setup();
  const controls = fixture.host.frameControls;
  const changed = jest.fn();
  const stop = controls.subscribe(changed);
  expect(controls.available).toBe(false);
  await controls.activate();
  expect(controls.action).toBe('add');
  fixture.host.ready(fixture.surface, true);
  expect(controls.available).toBe(false);
  await fixture.host.transition(true);
  expect(controls.available).toBe(true);
  expect(changed).toHaveBeenCalled();
  await controls.activate();
  expect(controls.action).toBe('expand');
  await fixture.host.transition(false);
  expect(controls.available).toBe(false);
  await controls.activate();
  expect(controls.action).toBe('expand');
  await fixture.host.transition(true);
  fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
  expect(controls.available).toBe(false);
  await controls.activate();
  expect(controls.action).toBe('expand');
  fixture.settings.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(true);
  expect(controls.available).toBe(true);
  fixture.host.release();
  expect(controls.available).toBe(false);
  fixture.host.ready(fixture.surface, false);
  expect(controls.available).toBe(true);
  expect(controls.action).toBe('expand');
  stop();
  changed.mockClear();
  fixture.host.release();
  expect(changed).not.toHaveBeenCalled();
});

test('density preferences fade only the material aperture and release it with the rail', async () => {
  const fixture = setup();
  fixture.host.setup();
  fixture.host.ready(fixture.surface, false);
  await nextTick();
  const workspace = fixture.surface.element.parentElement!;
  const control = workspace.querySelector('.sc-cube-stack-row__activate');
  expect(workspace.style.getPropertyValue('--sc-stack-wash-opacity')).toBe('1');
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(true);
  await nextTick();
  expect(workspace.style.getPropertyValue('--sc-stack-wash-opacity')).toBe('0');
  expect(workspace.style.getPropertyValue('--sc-stack-material-width')).toBe('58px');
  expect(workspace.querySelector('.sc-cube-stack-row__activate')).toBe(control);
  fixture.settings.get(CUBE_STACK_COMPACT_SETTING)?.onChange(false);
  await nextTick();
  expect(workspace.style.getPropertyValue('--sc-stack-wash-opacity')).toBe('1');
  expect(workspace.style.getPropertyValue('--sc-stack-material-width')).toBe('224px');
  fixture.host.release();
  expect(workspace.style.getPropertyValue('--sc-stack-wash-opacity')).toBe('');
  expect(workspace.style.getPropertyValue('--sc-stack-material-width')).toBe('');
});
