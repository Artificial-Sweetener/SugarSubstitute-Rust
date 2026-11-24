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
/** Frame presentation: the animated view icon, ComfyUI's sidebar tabs and the mirrored toolbar. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WORKFLOW_VIEW_ICON } from '../../../frontend/comfyui/ui/core/SharedElements.js';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import {
  WorkflowViewIcon,
  workflowViewBoxes,
  workflowViewLinkShare,
} from '../../../frontend/comfyui/ui/surface/masonry/WorkflowViewIcon.js';
import { ComfySidebarTabs } from '../../../frontend/comfyui/ui/surface/frame/ComfySidebarTabs.js';
import {
  GRAPH_TOOLBAR_MIRROR,
  graphToolbarCss,
} from '../../../frontend/comfyui/ui/surface/frame/GraphToolbarStyles.js';

afterEach(() => document.body.replaceChildren());

test('the view icon draws the graph at 0 and the masonry at 1, its links gone before it settles', () => {
  expect(workflowViewBoxes(0)).toEqual(WORKFLOW_VIEW_ICON.graph);
  expect(workflowViewBoxes(1)).toEqual(WORKFLOW_VIEW_ICON.masonry);
  expect(workflowViewLinkShare(0)).toBe(1);
  expect(workflowViewLinkShare(0.5)).toBe(0);
  const icon = new WorkflowViewIcon(document, 0);
  const links = [...icon.element.querySelectorAll('path')];
  const nodes = [...icon.element.querySelectorAll('rect')];
  expect(links).toHaveLength(3);
  expect(nodes).toHaveLength(4);
  expect(links.every((link) => link.getAttribute('visibility') === 'visible')).toBe(true);
  expect(links[0]?.getAttribute('stroke-width')).toBe(String(WORKFLOW_VIEW_ICON.link));
  icon.pose(1);
  expect(links.every((link) => link.getAttribute('visibility') === 'hidden')).toBe(true);
  // The first masonry tile, drawn inside its box by half an outline.
  expect(nodes[0]?.getAttribute('x')).toBe('3.5');
  expect(nodes[0]?.getAttribute('width')).toBe('5.5');
});

test('the view icon is cut off at its own 20 x 20 box in both views and while it moves', () => {
  // The approved graph draws its last node beyond the right edge, where the box cuts it off.
  const last = WORKFLOW_VIEW_ICON.graph.at(-1);
  expect(last?.[0]).toBeGreaterThan(20);
  for (const position of [0, 0.5, 1]) {
    const icon = new WorkflowViewIcon(document, position);
    expect(icon.element.getAttribute('viewBox')).toBe('0 0 20 20');
    expect(icon.element.style.overflow).toBe('hidden');
    icon.pose(1 - position);
    expect(icon.element.style.overflow).toBe('hidden');
  }
  // No stylesheet of the pack lets the icon paint outside its box.
  ensureWorkflowMasonryStyles(document);
  const rules = [...document.querySelectorAll('style')].map((style) => style.textContent).join('');
  expect(rules).toContain('.sugarcubes-masonry-toggle__icon');
  expect(rules).not.toMatch(/\.sugarcubes-masonry-toggle__icon[^}]*overflow/);
});

test('the view icon jumps to its destination when the viewer asks for reduced motion', () => {
  const original = window.matchMedia;
  window.matchMedia = jest.fn(
    () => ({ matches: true }) as MediaQueryList,
  ) as unknown as typeof window.matchMedia;
  try {
    const icon = new WorkflowViewIcon(document, 1);
    icon.animateTo(0);
    expect(icon.position).toBe(0);
  } finally {
    window.matchMedia = original;
  }
});

test('ComfyUI’s registered sidebar tabs are offered by their shown name and icon, and toggle', () => {
  const toggle = jest.fn();
  const store = {
    sidebarTabs: [
      { id: 'node-library', icon: 'icon-[comfy--node]', title: 'sideToolbar.nodeLibrary' },
      { id: 'my-extension', icon: { render: true }, title: 'My Panel' },
      { id: '', title: 'broken' },
    ],
    activeSidebarTabId: 'node-library',
    toggleSidebarTab: toggle,
  };
  const strip = document.createElement('button');
  strip.className = 'node-library-tab-button';
  strip.setAttribute('aria-label', 'Node Library (n)');
  const label = document.createElement('span');
  label.className = 'side-bar-button-label';
  label.textContent = 'No­des';
  strip.append(label);
  document.body.append(strip);
  const tabs = new ComfySidebarTabs(document, { extensionManager: { sidebarTab: store } });
  expect(tabs.tabs()).toEqual([
    { id: 'node-library', title: 'Nodes', icon: 'icon-[comfy--node]' },
    { id: 'my-extension', title: 'My Panel', icon: null },
  ]);
  expect(tabs.active()).toBe('node-library');
  tabs.toggle('my-extension');
  expect(toggle).toHaveBeenCalledWith('my-extension');
  expect(new ComfySidebarTabs(document, {}).tabs()).toEqual([]);
});

test('ComfyUI’s graph toolbar mirrors the canvas tab bar, in the SugarSubstitute frame only', () => {
  const css = graphToolbarCss.replace(/\s+/g, ' ');
  const { inset, padding, control, radius } = GRAPH_TOOLBAR_MIRROR;
  // The tab bar: 8 px from its edges, 4 px padding around 30 px tabs, 8 px corners.
  expect([inset, padding, control, radius]).toEqual([8, 4, 30, 8]);
  expect(css).toContain(
    `height:${String(control + 2 * padding + 2)}px; padding:${String(padding)}px`,
  );
  expect(css).toContain(`border-radius:${String(radius)}px`);
  expect(css).toContain(
    `bottom:calc(var(--sc-output-workspace-reserved-height,0px) + ${String(inset)}px)`,
  );
  for (const rule of css.split('}').filter((part) => part.includes('{')))
    expect(rule.trim().startsWith(':root[data-sc-substitute-frame]')).toBe(true);
});
