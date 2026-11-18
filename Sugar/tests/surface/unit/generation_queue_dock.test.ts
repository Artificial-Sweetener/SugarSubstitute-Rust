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
import { GenerationQueueDock } from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueDock.js';
import {
  GENERATION_QUEUE_DOCK_MINIMUM_WIDTH,
  GENERATION_QUEUE_DOCK_WIDTH_PROPERTY,
  GenerationQueueDockWidth,
} from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueDockWidth.js';
import {
  GENERATION_QUEUE_FLYOUT_WIDTH,
  GENERATION_QUEUE_LIST_SCALE,
  generationQueueSkinCss,
} from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueSkinStyles.js';

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

test('the docked queue is resized from its left edge, within bounds, and remembers its width', async () => {
  const stored = new Map<string, string>();
  const storage = {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => void stored.set(key, value),
  } as unknown as Storage;
  const root = document.documentElement;
  const width = () => root.style.getPropertyValue(GENERATION_QUEUE_DOCK_WIDTH_PROPERTY);
  const first = new GenerationQueueDockWidth(document, () => storage, jest.fn());
  expect(width()).toBe('360px');
  const drag = (from: number, to: number): void => {
    first.divider.dispatchEvent(new MouseEvent('pointerdown', { clientX: from, button: 0 }));
    // The pointer leaves the narrow edge at once; the rest of the drag is heard on the page.
    document.body.dispatchEvent(new MouseEvent('pointermove', { clientX: to, bubbles: true }));
    document.body.dispatchEvent(new MouseEvent('pointerup', { clientX: to, bubbles: true }));
  };
  // The queue is pinned at the right, so dragging its edge leftwards widens it.
  drag(900, 800);
  expect(width()).toBe('460px');
  expect(first.divider.getAttribute('aria-valuenow')).toBe('460');
  drag(800, 2000);
  expect(width()).toBe(`${String(GENERATION_QUEUE_DOCK_MINIMUM_WIDTH)}px`);
  // jsdom's window is 1024 px wide, and the queue never takes more than six tenths of it.
  drag(800, -2000);
  expect(width()).toBe('614px');
  first.divider.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
  expect(width()).toBe('598px');
  first.dispose();
  expect(width()).toBe('');

  const second = new GenerationQueueDockWidth(document, () => storage, jest.fn());
  expect(second.width).toBe(598);
  second.divider.dispatchEvent(new MouseEvent('dblclick'));
  expect(second.width).toBe(360);
  second.dispose();

  // A browser that refuses storage keeps the default and reports the refusal.
  const report = jest.fn();
  const refused = new GenerationQueueDockWidth(
    document,
    () => {
      throw new Error('storage refused');
    },
    report,
  );
  expect(refused.width).toBe(360);
  expect(report).toHaveBeenCalledTimes(1);
  refused.dispose();

  // Docked, the edge sits in the page's own layer, where Comfy's re-renders cannot drop it, and
  // leaves with the panel.
  const side = document.createElement('div');
  side.className = 'side-bar-panel';
  const panel = dockedPanel();
  side.append(panel);
  const dock = new GenerationQueueDock(document, {}, jest.fn());
  expect(document.querySelector('.sc-generation-queue-dock-divider')).toBeNull();
  document.body.append(side);
  await Promise.resolve();
  expect(document.querySelector('body > .sc-generation-queue-dock-divider')).not.toBeNull();
  expect(side.querySelector('.sc-generation-queue-dock-divider')).toBeNull();
  side.remove();
  await Promise.resolve();
  expect(document.querySelector('.sc-generation-queue-dock-divider')).toBeNull();
  dock.dispose();
  document.body.replaceChildren();
  window.localStorage.clear();
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

test('the queue fly-out is an opaque rounded panel, since the page cannot blur the canvas behind it', () => {
  const flyout =
    /\[data-testid="queue-progress-overlay"\] \{([^}]*)\}/.exec(generationQueueSkinCss)?.[1] ?? '';
  expect(flyout).toContain('background:var(--sc-surface-secondary,#2b2b2b)!important');
  expect(flyout).toContain('backdrop-filter:none!important');
  expect(flyout).toContain('border-radius:8px!important');
  expect(generationQueueSkinCss).not.toContain('blur(');
  expect(/background:[^;]*transparent/.test(flyout)).toBe(false);
});

test('the queue’s look applies only inside the frame and scales Comfy’s fixed row slots', () => {
  expect(GENERATION_QUEUE_FLYOUT_WIDTH).toBe(360);
  // Comfy positions rows in 48 px slots; the generation queue's row is 68 px with a 6 px gap.
  expect(48 * GENERATION_QUEUE_LIST_SCALE).toBeCloseTo(74);
  const selectors = generationQueueSkinCss
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('}')
    .map((rule) => rule.split('{')[0]?.trim() ?? '')
    .filter((selector) => selector && !selector.startsWith('@'));
  expect(selectors.length).toBeGreaterThan(20);
  for (const selector of selectors)
    expect(
      selector.includes(':root[data-sc-substitute-frame]') ||
        selector.startsWith('.sc-generation-queue-dock-hide'),
    ).toBe(true);
  // The queue button steps aside while the queue is docked, so it is never offered twice.
  expect(generationQueueSkinCss).toContain(
    ':root[data-sc-substitute-frame]:has([data-testid="job-history-sidebar"]) :is(button[data-sc-generation-queue],.sc-generation-queue-slot)',
  );
});
