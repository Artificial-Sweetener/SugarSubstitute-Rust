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
/** Prove native scrollbar layer ownership, zoom coordinates, and reversible resource lifetime. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  ScrollbarLayers,
  scrollbarCoordinateSpace,
} from '../../../frontend/comfyui/ui/surface/appearance/ScrollbarLayers.js';
import { ScrollbarView } from '../../../frontend/comfyui/ui/surface/appearance/ScrollbarView.js';
import { scrollbarTrack } from '../../../frontend/comfyui/ui/surface/appearance/ScrollbarLayout.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('PhotoPro retains the full scroll-owner track when its first arrow moves offscreen', () => {
  const owner = document.createElement('div');
  owner.style.overflow = 'auto';
  Object.defineProperties(owner, {
    clientWidth: { value: 200 },
    clientHeight: { value: 400 },
    offsetWidth: { value: 200 },
    offsetHeight: { value: 400 },
  });
  jest.spyOn(owner, 'getBoundingClientRect').mockReturnValue({
    x: 10,
    y: -120,
    left: 10,
    top: -120,
    right: 210,
    bottom: 280,
    width: 200,
    height: 400,
    toJSON: () => ({}),
  });
  document.body.append(owner);
  expect(scrollbarTrack(owner)?.top).toBe(0);
  expect(scrollbarTrack(owner, true)).toMatchObject({ top: -120, height: 400 });
});

test('measures all native layer boundaries before inserting any scrollbar layer', () => {
  const surface = document.createElement('div');
  surface.style.position = 'relative';
  const first = document.createElement('div');
  const second = document.createElement('div');
  surface.append(first, second);
  document.body.append(surface);
  const read = jest.spyOn(window, 'getComputedStyle');
  const write = jest.spyOn(surface, 'append');
  const layers = new ScrollbarLayers(document);
  const mounted = layers.mountMany(
    [first, second].map((owner) => ({
      owner,
      vertical: document.createElement('div'),
      horizontal: document.createElement('div'),
    })),
  );
  expect(mounted.get(first)).toBe(mounted.get(second));
  expect(Math.max(...read.mock.invocationCallOrder)).toBeLessThan(
    write.mock.invocationCallOrder[0]!,
  );
  layers.clear();
});

test('shares a positioned native surface until its final scroll owner is released', () => {
  const surface = document.createElement('div');
  surface.style.position = 'relative';
  const first = document.createElement('div'),
    second = document.createElement('div');
  surface.append(first, second);
  document.body.append(surface);
  const layers = new ScrollbarLayers(document);
  const firstThumb = document.createElement('div'),
    secondThumb = document.createElement('div');
  const layer = layers.mount(first, firstThumb, document.createElement('div'));
  expect(layers.mount(second, secondThumb, document.createElement('div'))).toBe(layer);
  const firstAttributes = jest.spyOn(first, 'getAttribute');
  layers.mount(second, secondThumb, document.createElement('div'));
  expect(firstAttributes).not.toHaveBeenCalled();
  expect(layer.parentElement).toBe(surface);
  expect(surface.style.position).toBe('relative');
  layers.release(first);
  expect(layer.isConnected).toBe(true);
  layers.release(second);
  expect(layer.isConnected).toBe(false);
});

test.each(['position', 'transform', 'contain'] as const)(
  'uses the nearest %s boundary and leaves the host unchanged',
  (property) => {
    const ancestor = document.createElement('div');
    ancestor.style.position = 'fixed';
    const owner = document.createElement('div');
    owner.style[property] =
      property === 'position' ? 'relative' : property === 'transform' ? 'scale(.5)' : 'paint';
    ancestor.append(owner);
    document.body.append(ancestor);
    const before = owner.getAttribute('style');
    const layers = new ScrollbarLayers(document);
    expect(
      layers.mount(owner, document.createElement('div'), document.createElement('div'))
        .parentElement,
    ).toBe(owner);
    layers.clear();
    expect(owner.getAttribute('style')).toBe(before);
    expect(owner.children).toHaveLength(0);
  },
);

test('keeps shadow thumbs in the shadow tree and supplies a body fallback for static light DOM', () => {
  const custom = document.createElement('div');
  const shadow = custom.attachShadow({ mode: 'open' });
  const owner = document.createElement('div');
  shadow.append(owner);
  const ordinary = document.createElement('div');
  document.body.append(custom, ordinary);
  const layers = new ScrollbarLayers(document);
  expect(
    layers.mount(owner, document.createElement('div'), document.createElement('div')).getRootNode(),
  ).toBe(shadow);
  expect(
    layers.mount(ordinary, document.createElement('div'), document.createElement('div'))
      .parentElement,
  ).toBe(document.body);
  layers.clear();
  expect(shadow.querySelector('.sc-style-scrollbar-layer')).toBeNull();
});

test.each(['flex', 'inline-flex', 'grid', 'inline-grid'])(
  'keeps overlays above controls inside a static %s item with its own stacking context',
  (display) => {
    const bar = document.createElement('div');
    bar.style.cssText = `position:relative;display:${display}`;
    const toolbar = document.createElement('div');
    toolbar.style.zIndex = '1200';
    const owner = document.createElement('div');
    toolbar.append(owner);
    bar.append(toolbar);
    document.body.append(bar);
    const before = toolbar.getAttribute('style');
    const layers = new ScrollbarLayers(document);
    const layer = layers.mount(owner, document.createElement('div'), document.createElement('div'));
    expect(layer.parentElement).toBe(toolbar);
    expect(toolbar.getAttribute('style')).toBe(before);
    layers.clear();
    expect(toolbar.children).toHaveLength(1);
  },
);

test('maps viewport geometry through a zoomed and scrolled native layer while preserving screen thumb size', () => {
  const layer = document.createElement('div');
  Object.defineProperties(layer, { offsetWidth: { value: 400 }, offsetHeight: { value: 600 } });
  jest.spyOn(layer, 'getBoundingClientRect').mockReturnValue({
    x: 100,
    y: -50,
    left: 100,
    top: -50,
    right: 300,
    bottom: 250,
    width: 200,
    height: 300,
    toJSON: () => ({}),
  });
  const space = scrollbarCoordinateSpace(layer);
  expect(space).toEqual({ left: 100, top: -50, scaleX: 0.5, scaleY: 0.5 });
  const owner = document.createElement('div');
  owner.style.overflow = 'auto';
  Object.defineProperties(owner, {
    clientWidth: { value: 300 },
    clientHeight: { value: 200 },
    scrollWidth: { value: 300 },
    scrollHeight: { value: 1000 },
  });
  const view = new ScrollbarView(owner, 'vertical', 'owner', jest.fn());
  const commit = view.prepare(
    {
      left: 110,
      top: 20,
      width: 150,
      height: 100,
      horizontalBottom: 120,
      verticalEdge: 260,
      rtl: false,
    },
    space,
  );
  expect(view.element.style.left).toBe('');
  commit();
  expect(view.element.style.left).toBe('296px');
  expect(view.element.style.top).toBe('142px');
  expect(view.element.style.width).toBe('24px');
  expect(view.element.style.height).toBe('48px');
  expect(view.element.style.getPropertyValue('--sc-scrollbar-scale-x')).toBe('0.5');
  expect(view.element.style.getPropertyValue('--sc-scrollbar-scale-y')).toBe('0.5');
  view.dispose();
});
