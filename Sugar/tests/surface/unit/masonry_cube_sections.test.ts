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
/** Prove a masonry scroll is kept by its Cube and lands there again after the cards reflow. */
import { afterEach, expect, test } from '@jest/globals';
import {
  captureMasonryScroll,
  masonryCubeSections,
  masonryScrollTop,
} from '../../../frontend/comfyui/ui/surface/masonry/MasonryCubeSections.js';
import { MasonryScrollKeeper } from '../../../frontend/comfyui/ui/surface/masonry/MasonryScrollKeeper.js';
import type { WorkflowScroll } from '../../../frontend/comfyui/ui/workflow/domain/WorkflowView.js';

afterEach(() => document.body.replaceChildren());

/** A masonry with settled section headers at the given layout positions. */
function masonry(sections: readonly [string, number, number][]): HTMLElement {
  const element = document.createElement('div');
  for (const [section, y, height] of sections) {
    const header = document.createElement('header');
    header.dataset.masonrySection = section;
    header.dataset.masonryY = String(y);
    header.dataset.masonryHeight = String(height);
    element.append(header);
  }
  document.body.append(element);
  return element;
}

test('a scroll is kept by the Cube holding the top of the view', () => {
  const element = masonry([
    ['cube:9', 600, 400],
    ['loose:1', 0, 100],
    ['cube:4', 100, 500],
  ]);
  expect(masonryCubeSections(element).map((section) => section.id)).toEqual(['4', '9']);
  element.scrollTop = 0;
  expect(captureMasonryScroll(element)).toEqual({ cube: null, offset: 0 });
  element.scrollTop = 250;
  expect(captureMasonryScroll(element)).toEqual({ cube: '4', offset: 150 });
  element.scrollTop = 640;
  expect(captureMasonryScroll(element)).toEqual({ cube: '9', offset: 40 });
});

test('a kept scroll lands the same distance into its Cube after the cards reflow', () => {
  const element = masonry([
    ['cube:4', 100, 300],
    ['cube:9', 420, 400],
  ]);
  expect(masonryScrollTop(element, { cube: '9', offset: 40 })).toBe(460);
  expect(masonryScrollTop(element, { cube: null, offset: 30 })).toBe(30);
  // A Cube not laid out yet has no place to land.
  expect(masonryScrollTop(element, { cube: 'later', offset: 40 })).toBeNull();
});

/** A masonry whose reachable scroll grows as its cards grow, as a browser clamps scrollTop. */
function growing(sections: readonly [string, number, number][], reach: number) {
  const element = masonry(sections);
  // A laid out masonry: jsdom gives elements no height of their own.
  Object.defineProperty(element, 'clientHeight', { get: () => (element.isConnected ? 700 : 0) });
  let top = 0;
  const state = { reach };
  Object.defineProperty(element, 'scrollTop', {
    get: () => top,
    set: (value: number) => {
      top = Math.max(0, Math.min(value, state.reach));
    },
  });
  return { element, state };
}

function memory(kept: WorkflowScroll | null) {
  const recorded: WorkflowScroll[] = [];
  return {
    recorded,
    read: () => kept,
    record: (scroll: WorkflowScroll) => {
      recorded.push(scroll);
    },
    subscribe: () => () => undefined,
  };
}

test('a kept scroll the cards cannot reach yet waits for them, and nothing overwrites it meanwhile', () => {
  const { element, state } = growing([['cube:4', 0, 900]], 100);
  const kept = memory({ cube: '4', offset: 319 });
  const keeper = new MasonryScrollKeeper(element, kept, () => undefined);
  keeper.settle();
  expect(element.scrollTop).toBe(100);
  element.dispatchEvent(new Event('scroll'));
  element.dispatchEvent(new Event('scrollend'));
  expect(kept.recorded).toEqual([]);
  // The cards grow; the next layout lands the kept scroll and records it.
  state.reach = 600;
  element.dispatchEvent(new CustomEvent('sc-masonry-layout'));
  expect(element.scrollTop).toBe(319);
  expect(kept.recorded).toEqual([{ cube: '4', offset: 319 }]);
  keeper.dispose();
});

test('a person scrolling takes over from a kept scroll still waiting for the cards', () => {
  const { element, state } = growing([['cube:4', 0, 900]], 100);
  const kept = memory({ cube: '4', offset: 319 });
  const keeper = new MasonryScrollKeeper(element, kept, () => undefined);
  keeper.settle();
  element.dispatchEvent(new Event('wheel'));
  element.scrollTop = 40;
  state.reach = 600;
  element.dispatchEvent(new CustomEvent('sc-masonry-layout'));
  expect(element.scrollTop).toBe(40);
  element.dispatchEvent(new Event('scrollend'));
  expect(kept.recorded).toEqual([{ cube: '4', offset: 40 }]);
  keeper.dispose();
});

test('a scroll made while the cards entered wins over the kept one', () => {
  const { element } = growing([['cube:4', 0, 900]], 600);
  const kept = memory({ cube: '4', offset: 319 });
  const keeper = new MasonryScrollKeeper(element, kept, () => undefined);
  element.scrollTop = 90;
  keeper.settle();
  expect(element.scrollTop).toBe(90);
  expect(kept.recorded).toEqual([{ cube: '4', offset: 90 }]);
  keeper.dispose();
});

test('a masonry taken out of the document never records the top it reads there', () => {
  const { element } = growing([['cube:4', 0, 900]], 600);
  const kept = memory({ cube: '4', offset: 319 });
  const keeper = new MasonryScrollKeeper(element, kept, () => undefined);
  keeper.settle();
  expect(kept.recorded).toEqual([{ cube: '4', offset: 319 }]);
  // Closing removes the masonry before its keeper is disposed; it then reads as scrolled to 0.
  element.remove();
  element.scrollTop = 0;
  keeper.dispose();
  expect(kept.recorded).toEqual([{ cube: '4', offset: 319 }]);
});

test('a scroll a restored session brings moves a masonry whose cards have entered', () => {
  const { element } = growing([['cube:4', 0, 900]], 600);
  let kept: WorkflowScroll | null = { cube: '4', offset: 100 };
  let restore: () => void = () => undefined;
  const recorded: WorkflowScroll[] = [];
  const keeper = new MasonryScrollKeeper(
    element,
    {
      read: () => kept,
      record: (scroll) => {
        recorded.push(scroll);
      },
      subscribe: (listener) => {
        restore = listener;
        return () => undefined;
      },
    },
    () => undefined,
  );
  keeper.settle();
  expect(element.scrollTop).toBe(100);
  kept = { cube: '4', offset: 319 };
  restore();
  expect(element.scrollTop).toBe(319);
  expect(recorded.at(-1)).toEqual({ cube: '4', offset: 319 });
  keeper.dispose();
});
