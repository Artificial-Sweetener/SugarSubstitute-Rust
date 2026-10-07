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
/** Prove graph-to-masonry endpoint motion, reversal, visibility bounds, and cleanup. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryTransition } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryTransition.js';
import type { WorkflowMasonryGraphSnapshot } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphGeometry.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Bind real presentation elements to deterministic browser animation completion. */
function fixture(sourceX = 40, destinationX = 320) {
  const surface = document.createElement('section');
  const backdrop = document.createElement('div');
  const cell = document.createElement('article');
  const element = cell;
  const content = document.createElement('div');
  const source = document.createElement('article');
  source.style.width = '200px';
  const layer = document.createElement('div');
  cell.append(content);
  surface.append(cell);
  layer.append(source);
  document.body.append(layer, backdrop, surface);
  jest.spyOn(cell, 'getBoundingClientRect').mockReturnValue(rectangle(destinationX, 80, 320, 200));
  const effects: Array<{
    element: HTMLElement;
    frames: Keyframe[];
    finish(): void;
    cancel: ReturnType<typeof jest.fn>;
  }> = [];
  for (const target of [element, backdrop, layer, source]) {
    Object.defineProperty(target, 'animate', {
      configurable: true,
      value: (frames: Keyframe[]) => {
        let finish = (): void => undefined;
        let reject = (_error: Error): void => undefined;
        const finished = new Promise<void>((resolve, failure) => {
          finish = resolve;
          reject = failure;
        });
        const effect = {
          element: target,
          frames,
          finished,
          finish: () => finish(),
          cancel: jest.fn(() => reject(new Error('Cancelled'))),
        };
        effects.push(effect);
        return effect;
      },
    });
  }
  const graph: WorkflowMasonryGraphSnapshot = {
    viewport: rectangle(0, 0, 1000, 800),
    layers: [layer],
    cards: new Map([
      [
        'root:1',
        {
          element: source,
          rectangle: rectangle(sourceX, 200, 160, 100),
          layoutWidth: 200,
          scale: 0.8,
          visible: sourceX < 1000,
        },
      ],
    ]),
  };
  return {
    surface,
    backdrop,
    element,
    content,
    source,
    layer,
    cell,
    effects,
    transition: new WorkflowMasonryTransition(),
    options: { surface, backdrop, cards: [{ id: 'root:1', cell }], graph },
  };
}

test('assembles actual cards from graph bounds and returns them to those bounds', async () => {
  const setup = fixture();
  const entering = setup.transition.run(setup.options, true, false);
  expect(setup.effects.find((effect) => effect.element === setup.element)?.frames).toEqual([
    { transform: 'translate(-280px, 120px) scale(0.8)', width: '200px', opacity: 1 },
    { transform: 'translate(0px, 0px) scale(1)', width: '320px', opacity: 1 },
  ]);
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(true);
  for (const effect of setup.effects) effect.finish();
  await entering;
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(false);
  expect(setup.element.hasAttribute('data-sc-masonry-moving')).toBe(false);
  const exit = setup.transition.run(setup.options, false, false);
  expect(setup.effects.filter((effect) => effect.element === setup.element).at(-1)?.frames).toEqual(
    [
      { transform: 'translate(0px, 0px) scale(1)', width: '320px', opacity: 1 },
      { transform: 'translate(-280px, 120px) scale(0.8)', width: '200px', opacity: 1 },
    ],
  );
  for (const effect of setup.effects) effect.finish();
  await exit;
  expect(setup.backdrop.style.opacity).toBe('0');
  expect(setup.cell.style.transform).toBe('');
});

test('moves the complete painted card from its settled placement without leaving a stationary shell', async () => {
  const setup = fixture();
  setup.cell.style.cssText =
    'transform:translate(24px, 44px);background:rgb(32,32,32);border:1px solid gray';
  const entry = setup.transition.run(setup.options, true, false);
  try {
    expect(setup.cell.hasAttribute('data-sc-masonry-moving')).toBe(true);
    expect(setup.content.hasAttribute('data-sc-masonry-moving')).toBe(false);
    expect(setup.effects.at(-1)?.frames).toEqual([
      {
        transform: 'translate(24px, 44px) translate(-280px, 120px) scale(0.8)',
        width: '200px',
        opacity: 1,
      },
      { transform: 'translate(24px, 44px)', width: '320px', opacity: 1 },
    ]);
  } finally {
    setup.transition.cancel();
    await entry;
  }
  expect(setup.cell.style.transform).toBe('translate(24px, 44px)');
});

/** Model the browser geometry boundary without relying on JSDOM's absent DOMRect constructor. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    top: y,
    left: x,
    right: x + width,
    bottom: y + height,
    toJSON: () => null,
  };
}

test('reverses from the displayed transform and cancels every stale effect', async () => {
  const setup = fixture();
  const entering = setup.transition.run(setup.options, true, false);
  const interrupted = setup.effects.slice();
  setup.element.style.transform = 'matrix(0.8, 0, 0, 0.8, -100, 40)';
  setup.element.style.width = '260px';
  const exit = setup.transition.run(setup.options, false, false);
  await entering;
  expect(
    setup.effects.filter((effect) => effect.element === setup.element).at(-1)?.frames[0]?.transform,
  ).toBe('matrix(0.8, 0, 0, 0.8, -100, 40)');
  expect(
    setup.effects.filter((effect) => effect.element === setup.element).at(-1)?.frames[0]?.width,
  ).toBe('260px');
  expect(interrupted.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  setup.transition.cancel();
  await exit;
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(false);
  expect(setup.surface.hasAttribute('data-sc-masonry-transition')).toBe(false);
});

test('allocates no moving-card effects for endpoints outside the viewport or reduced motion', async () => {
  const setup = fixture(2000, 3000);
  const entry = setup.transition.run(setup.options, true, false);
  expect(setup.effects.some((effect) => effect.element === setup.element)).toBe(false);
  setup.transition.cancel();
  await entry;
  setup.effects.length = 0;
  await setup.transition.run(setup.options, true, true);
  expect(setup.effects).toHaveLength(0);
  expect(setup.backdrop.style.opacity).toBe('1');
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(false);
});

test('fades an unmatched card without inventing a graph position', async () => {
  const setup = fixture();
  setup.options.graph.cards.clear();
  const entry = setup.transition.run(setup.options, true, false);
  expect(setup.effects.find((effect) => effect.element === setup.element)?.frames).toEqual([
    { opacity: 0 },
    { opacity: 1 },
  ]);
  setup.transition.cancel();
  await entry;
});

test('releases already-created effects when the browser rejects another animation', async () => {
  const setup = fixture();
  Object.defineProperty(setup.element, 'animate', {
    value: () => {
      throw new Error('Native animation failed');
    },
  });
  await expect(setup.transition.run(setup.options, true, false)).rejects.toThrow(
    'Native animation failed',
  );
  expect(setup.effects.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(false);
  expect(setup.surface.hasAttribute('data-sc-masonry-transition')).toBe(false);
});

test('blends the real loose graph card during travel and restores its geometry identities on completion', async () => {
  const setup = fixture();
  setup.source.className = 'lg-node';
  setup.source.dataset.nodeId = '1';
  const grid = document.createElement('div');
  grid.dataset.widgetsGridNodeId = '1';
  setup.source.append(grid);
  const entry = setup.transition.run(setup.options, true, false);
  expect(setup.source.dataset.nodeId).toBeUndefined();
  expect(grid.dataset.widgetsGridNodeId).toBeUndefined();
  expect(setup.source.dataset.scMasonryGraphNode).toBe('1');
  expect(setup.source.hasAttribute('data-sc-masonry-source')).toBe(false);
  const projected = setup.effects.filter((effect) => effect.element === setup.element).at(-1)!;
  expect(projected.frames.map((frame) => frame.opacity)).toEqual([0, 1]);
  expect(projected.frames[0]?.transform).toBe('translate(-280px, 120px) scale(0.8)');
  for (const effect of setup.effects) effect.finish();
  await entry;
  expect(setup.source.dataset.nodeId).toBe('1');
  expect(grid.dataset.widgetsGridNodeId).toBe('1');
  expect(setup.source.dataset.scMasonryGraphNode).toBeUndefined();
  expect(setup.source.style.transformOrigin).toBe('');
});

test('interpolates loose graph and masonry heights together across native minimum-height constraints', async () => {
  const setup = fixture();
  setup.source.className = 'lg-node';
  setup.source.style.minHeight = '125px';
  const entry = setup.transition.run(setup.options, true, false);
  try {
    const graph = setup.effects.find((effect) => effect.element === setup.source);
    const masonry = setup.effects.find((effect) => effect.element === setup.cell);
    expect(graph?.frames.map((frame) => frame.height)).toEqual(['125px', '200px']);
    expect(masonry?.frames.map((frame) => frame.height)).toEqual(['125px', '200px']);
    expect(
      graph?.frames.every((frame) => frame.minHeight === '0px' && frame.overflow === 'clip'),
    ).toBe(true);
  } finally {
    setup.transition.cancel();
    await entry;
  }
  expect(setup.source.style.minHeight).toBe('125px');
  expect(setup.source.style.height).toBe('');
});

test('reverses a loose card from its displayed width without replacing its original graph endpoint', async () => {
  const setup = fixture();
  setup.source.className = 'lg-node';
  setup.source.dataset.nodeId = '1';
  setup.source.style.transform = 'translate(20px, 40px)';
  setup.source.style.setProperty('transform-origin', '50% 50%', 'important');
  const entry = setup.transition.run(setup.options, true, false);
  setup.source.style.width = '260px';
  setup.source.style.height = '150px';
  setup.source.style.transform = 'matrix(1.1, 0, 0, 1.1, 100, 80)';
  setup.options.graph.cards.set('root:1', {
    element: setup.source,
    rectangle: rectangle(200, 150, 286, 110),
    layoutWidth: 260,
    scale: 1.1,
    visible: true,
  });
  const exit = setup.transition.run(setup.options, false, false);
  await entry;
  const graphEffect = setup.effects.filter((effect) => effect.element === setup.source).at(-1)!;
  expect(graphEffect.frames[0]).toEqual({
    transform: 'matrix(1.1, 0, 0, 1.1, 100, 80)',
    width: '260px',
    height: '150px',
    minHeight: '0px',
    overflow: 'clip',
  });
  expect(graphEffect.frames[1]).toEqual({
    transform: 'translate(20px, 40px)',
    width: '200px',
    height: '125px',
    minHeight: '0px',
    overflow: 'clip',
  });
  expect(setup.source.dataset.nodeId).toBeUndefined();
  setup.transition.cancel();
  await exit;
  expect(setup.source.dataset.nodeId).toBe('1');
  expect(setup.source.style.transformOrigin).toBe('50% 50%');
  expect(setup.source.style.getPropertyPriority('transform-origin')).toBe('important');
  expect(document.querySelector('[data-sc-geometry-moving]')).toBeNull();
});

test('restores loose graph identities and releases moving regions when a later animation fails', async () => {
  const setup = fixture();
  setup.source.className = 'lg-node';
  setup.source.dataset.nodeId = '1';
  Object.defineProperty(setup.element, 'animate', {
    value: () => {
      throw new Error('Animation failed');
    },
  });
  await expect(setup.transition.run(setup.options, true, false)).rejects.toThrow(
    'Animation failed',
  );
  expect(setup.source.dataset.nodeId).toBe('1');
  expect(
    document.querySelector('[data-sc-geometry-moving], [data-sc-masonry-graph-node]'),
  ).toBeNull();
  expect(setup.effects.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
});

test('does not mutate stable surface styles and trigger native subtree rediscovery on exit', async () => {
  const setup = fixture();
  setup.surface.style.opacity = '1';
  const observer = new MutationObserver(() => undefined);
  observer.observe(setup.surface, { attributes: true, attributeFilter: ['style'] });
  const exit = setup.transition.run(setup.options, false, false);
  expect(observer.takeRecords()).toHaveLength(0);
  setup.transition.cancel();
  await exit;
  observer.disconnect();
});

test('batches annotations before starting effects instead of invalidating style between cards', async () => {
  const setup = fixture();
  const writes: number[] = [];
  const observer = new MutationObserver(() => undefined);
  observer.observe(document.body, { attributes: true, subtree: true });
  for (const element of [setup.backdrop, setup.layer, setup.element]) {
    const original = element.animate;
    Object.defineProperty(element, 'animate', {
      value: (frames: Keyframe[], options: KeyframeAnimationOptions) => {
        writes.push(observer.takeRecords().length);
        return original.call(element, frames, options);
      },
    });
  }
  const entry = setup.transition.run(setup.options, true, false);
  expect(writes[0]).toBeGreaterThan(0);
  expect(writes.slice(1)).toEqual([0, 0]);
  setup.transition.cancel();
  await entry;
  observer.disconnect();
});
