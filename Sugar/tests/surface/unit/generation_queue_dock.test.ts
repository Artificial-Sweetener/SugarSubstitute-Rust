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
/** Prove the docked generation queue can be put away, and that its look is scoped to the frame. */
import { expect, jest, test } from '@jest/globals';
import {
  GENERATION_QUEUE_DOCKED_ATTRIBUTE,
  GenerationQueueDock,
} from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueDock.js';
import { generationQueueSkinCss } from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueSkinStyles.js';
import {
  GENERATION_QUEUE_FLYOUT_MAX_HEIGHT,
  GENERATION_QUEUE_FLYOUT_WIDTH,
  generationQueueViewCss,
} from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueViewStyles.js';

/** Build Comfy's docked job panel the way it mounts: a side panel holding a header toolbar. */
function dockedPanel(): HTMLElement {
  const panel = document.createElement('div');
  panel.dataset.testid = 'job-history-sidebar';
  const start = document.createElement('div');
  start.className = 'p-toolbar-start';
  const title = document.createElement('span');
  title.textContent = 'Job History';
  const actions = document.createElement('div');
  actions.className = 'ml-auto';
  start.append(title, actions);
  panel.append(start);
  return panel;
}

test('the docked queue carries a button that closes Comfy’s own panel, and loses it with the panel', async () => {
  const tabs = {
    activeSidebarTabId: 'job-history' as string | null,
    toggleSidebarTab: jest.fn((id: string) => {
      tabs.activeSidebarTabId = tabs.activeSidebarTabId === id ? null : id;
    }),
  };
  const report = jest.fn();
  const dock = new GenerationQueueDock(
    document,
    { extensionManager: { sidebarTab: tabs } },
    report,
  );
  expect(document.querySelector('.sc-generation-queue-dock-hide')).toBeNull();

  const panel = dockedPanel();
  document.body.append(panel);
  await Promise.resolve();
  const hide = panel.querySelector<HTMLButtonElement>('.ml-auto > .sc-generation-queue-dock-hide');
  expect(hide?.getAttribute('aria-label')).toBe('Hide generation queue');

  hide?.click();
  expect(tabs.toggleSidebarTab).toHaveBeenCalledWith('job-history');
  // A panel that is no longer the shown tab is not toggled back open.
  hide?.click();
  expect(tabs.toggleSidebarTab).toHaveBeenCalledTimes(1);

  panel.remove();
  await Promise.resolve();
  expect(document.querySelector('.sc-generation-queue-dock-hide')).toBeNull();
  dock.dispose();
  document.body.append(dockedPanel());
  await Promise.resolve();
  expect(document.querySelector('.sc-generation-queue-dock-hide')).toBeNull();
  expect(report).not.toHaveBeenCalled();
  document.body.replaceChildren();
});

test('the docked queue is not resizable: no edge of its own, and Comfy\u2019s splitter edge beside it hidden', async () => {
  const style = document.createElement('style');
  style.textContent = generationQueueSkinCss;
  document.head.append(style);
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  // Comfy's splitter: the side panel, then its resizing edge, then the graph.
  const splitter = document.createElement('div');
  splitter.className = 'p-splitter';
  const side = document.createElement('div');
  side.className = 'side-bar-panel';
  const gutter = document.createElement('div');
  gutter.className = 'p-splitter-gutter';
  const graph = document.createElement('div');
  graph.className = 'p-splitterpanel';
  splitter.append(side, gutter, graph);
  document.body.append(splitter);
  const dock = new GenerationQueueDock(document, {}, jest.fn());
  // Another side panel keeps Comfy's edge.
  expect(getComputedStyle(gutter).display).not.toBe('none');
  side.append(dockedPanel());
  await Promise.resolve();
  expect(document.documentElement.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE)).toBe(true);
  expect(getComputedStyle(gutter).display).toBe('none');
  expect(
    document.querySelector(
      '[class*="dock-divider"], [aria-orientation="vertical"][role="separator"]',
    ),
  ).toBeNull();
  // The panel sits at the fly-out's width.
  expect(style.textContent).toContain(
    `--sc-queue-dock:${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px;`,
  );
  dock.dispose();
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  document.head.replaceChildren();
  document.body.replaceChildren();
});

test('a failing Comfy store is reported and never thrown at the person', () => {
  const report = jest.fn();
  const failure = new Error('store unavailable');
  const tabs = {
    activeSidebarTabId: 'job-history',
    toggleSidebarTab: () => {
      throw failure;
    },
  };
  const panel = dockedPanel();
  document.body.append(panel);
  const dock = new GenerationQueueDock(
    document,
    { extensionManager: { sidebarTab: tabs } },
    report,
  );
  panel.querySelector<HTMLButtonElement>('.sc-generation-queue-dock-hide')?.click();
  expect(report).toHaveBeenCalledWith(failure);
  dock.dispose();
  document.body.replaceChildren();
});

test('inside the frame ComfyUI’s own queue panels give way to the generation queue’s view', () => {
  const rules = generationQueueSkinCss.replace(/\s+/g, ' ');
  expect(rules).toContain(
    ':root[data-sc-substitute-frame] [data-testid="queue-progress-overlay"] { display:none!important; }',
  );
  // The pinned panel keeps its place beside the canvas; only the frame's view shows in it.
  expect(rules).toContain('> :not(.sc-generation-queue) { display:none!important; }');
  // The queue button steps aside while the queue is pinned, so it is never offered twice.
  expect(generationQueueSkinCss).toContain(
    ':root[data-sc-substitute-frame][data-sc-queue-docked] :is(button[data-sc-generation-queue],.sc-generation-queue-slot)',
  );
  const selectors = generationQueueSkinCss
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('}')
    .map((rule) => rule.split('{')[0]?.trim() ?? '')
    .filter((selector) => selector && !selector.startsWith('@'));
  for (const selector of selectors)
    expect(selector.includes(':root[data-sc-substitute-frame]')).toBe(true);
});

test('the queue keeps SugarSubstitute’s measurements in every visual style', () => {
  expect(GENERATION_QUEUE_FLYOUT_WIDTH).toBe(360);
  expect(GENERATION_QUEUE_FLYOUT_MAX_HEIGHT).toBe(420);
  const rules = generationQueueViewCss.replace(/\s+/g, ' ');
  // A row is the 52 px thumbnail and 8 px above and below it, six pixels from the next.
  expect(rules).toContain('flex:0 0 68px; height:68px;');
  expect(rules).toContain('flex:0 0 52px; width:52px; height:52px;');
  expect(rules).toContain(
    '.sc-generation-queue-rows { display:flex; flex-direction:column; gap:6px;',
  );
  expect(rules).toContain(
    "[data-surface='flyout'] { width:360px; max-height:420px; padding:12px 14px 14px;",
  );
  // Colours and fonts come from the active style's tokens, never from one style's values.
  expect(rules).toContain('font-family:var(--sc-font-family)');
  expect(rules).toContain('background:var(--sc-surface-secondary');
  expect(rules).not.toContain('blur(');
});
