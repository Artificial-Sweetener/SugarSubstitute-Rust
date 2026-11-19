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
/** Prove the tab bar previews a tab that would open, where it would open, and puts itself back. */
import { afterEach, expect, test } from '@jest/globals';
import { WorkflowTabGhost } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabGhost.js';

afterEach(() => document.body.replaceChildren());

/** A bar from x=0 to 900 holding a strip that starts at x=100 with three 180 px tabs. */
function fixture(scrollLeft = 0) {
  const bar = document.createElement('header');
  const viewport = document.createElement('div');
  bar.getBoundingClientRect = () =>
    ({ left: 0, right: 900, top: 0, bottom: 64, width: 900, height: 64 }) as DOMRect;
  viewport.getBoundingClientRect = () =>
    ({ left: 100, right: 640, top: 0, bottom: 30, width: 540, height: 30 }) as DOMRect;
  viewport.scrollLeft = scrollLeft;
  const tabs = ['a', 'b', 'c'].map(() => document.createElement('div'));
  viewport.append(...tabs);
  bar.append(viewport);
  document.body.append(bar);
  const ghost = new WorkflowTabGhost(
    document,
    viewport,
    bar,
    () => tabs,
    () => [180, 180, 180],
  );
  return { bar, tabs, ghost, shown: () => bar.querySelector<HTMLElement>('.sc-workflow-tab-ghost') };
}

test('a point on the bar names the tab position a drop would take', () => {
  const f = fixture();
  // Before the first tab's middle, between tabs, and past the last tab.
  expect(f.ghost.hit(120, 15)).toBe(0);
  expect(f.ghost.hit(300, 15)).toBe(1);
  expect(f.ghost.hit(480, 15)).toBe(2);
  expect(f.ghost.hit(880, 15)).toBe(3);
  // The toolbar row below the tabs is not the tab bar.
  expect(f.ghost.hit(300, 50)).toBeNull();
  expect(f.ghost.hit(950, 15)).toBeNull();
  // A scrolled strip is measured from what is scrolled out of view.
  expect(fixture(180).ghost.hit(120, 15)).toBe(1);
});

test('the ghost shows the title literally where the tab would sit and moves later tabs aside', () => {
  const f = fixture();
  f.ghost.show('<b>Portrait</b>', 1);
  const ghost = f.shown()!;
  expect(ghost.textContent).toBe('<b>Portrait</b>');
  expect(ghost.querySelector('b')).toBeNull();
  expect(ghost.style.left).toBe('280px');
  expect(ghost.style.width).toBe('180px');
  expect(f.tabs.map((tab) => tab.style.translate || '')).toEqual(['', '180px 0', '180px 0']);
  f.ghost.show('<b>Portrait</b>', 3);
  expect(f.bar.querySelectorAll('.sc-workflow-tab-ghost')).toHaveLength(1);
  expect(f.shown()!.style.left).toBe('640px');
  expect(f.tabs.map((tab) => tab.style.translate || '')).toEqual(['', '', '']);
  f.ghost.show(null, 0);
  expect(f.shown()).toBeNull();
  f.ghost.show('Again', 0);
  f.ghost.dispose();
  expect(f.shown()).toBeNull();
  expect(f.tabs.map((tab) => tab.style.translate || '')).toEqual(['', '', '']);
});
