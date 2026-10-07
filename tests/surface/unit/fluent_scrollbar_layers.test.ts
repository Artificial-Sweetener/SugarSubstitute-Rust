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
  FluentScrollbarLayers,
  scrollbarCoordinateSpace,
} from '../../../frontend/comfyui/ui/surface/appearance/FluentScrollbarLayers.js';
import { FluentScrollbarView } from '../../../frontend/comfyui/ui/surface/appearance/FluentScrollbarView.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('shares a positioned native surface until its final scroll owner is released', () => {
  const surface = document.createElement('div');
  surface.style.position = 'relative';
  const first = document.createElement('div'),
    second = document.createElement('div');
  surface.append(first, second);
  document.body.append(surface);
  const layers = new FluentScrollbarLayers(document);
  const firstThumb = document.createElement('div'),
    secondThumb = document.createElement('div');
  const layer = layers.mount(first, firstThumb, document.createElement('div'));
  expect(layers.mount(second, secondThumb, document.createElement('div'))).toBe(layer);
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
    const layers = new FluentScrollbarLayers(document);
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
  const layers = new FluentScrollbarLayers(document);
  expect(
    layers.mount(owner, document.createElement('div'), document.createElement('div')).getRootNode(),
  ).toBe(shadow);
  expect(
    layers.mount(ordinary, document.createElement('div'), document.createElement('div'))
      .parentElement,
  ).toBe(document.body);
  layers.clear();
  expect(shadow.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
});

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
  const view = new FluentScrollbarView(owner, 'vertical', 'owner', jest.fn());
  view.update({ left: 110, top: 20, width: 150, height: 100, rtl: false }, space);
  expect(view.element.style.left).toBe('296px');
  expect(view.element.style.top).toBe('142px');
  expect(view.element.style.width).toBe('24px');
  expect(view.element.style.height).toBe('48px');
  expect(view.element.style.getPropertyValue('--sc-scrollbar-scale-x')).toBe('0.5');
  expect(view.element.style.getPropertyValue('--sc-scrollbar-scale-y')).toBe('0.5');
  view.dispose();
});
