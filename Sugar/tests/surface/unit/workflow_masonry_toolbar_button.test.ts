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
/** Prove icon-only mode controls follow the native queue and Properties toolbar lifecycle. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryToolbarButton } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryToolbarButton.js';

afterEach(() => document.body.replaceChildren());

/** Reproduce the host's nested queue and Properties tooltip slots. */
function toolbar(): { bar: HTMLElement; queueSlot: HTMLElement; propertiesSlot: HTMLElement } {
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const queueSlot = document.createElement('div');
  const queue = document.createElement('button');
  queue.dataset.testid = 'queue-overlay-toggle';
  queueSlot.append(queue);
  const propertiesSlot = document.createElement('div');
  const properties = document.createElement('button');
  const icon = document.createElement('i');
  icon.className = 'icon-[lucide--panel-right]';
  properties.append(icon);
  propertiesSlot.append(properties);
  bar.append(queueSlot, propertiesSlot);
  document.body.append(bar);
  return { bar, queueSlot, propertiesSlot };
}

test('places one icon-only switch between queue and Properties and changes its destination', () => {
  const { bar, queueSlot, propertiesSlot } = toolbar();
  const toggle = jest.fn();
  const control = new WorkflowMasonryToolbarButton(document, toggle);
  control.setup();
  const button = bar.querySelector<HTMLButtonElement>('.sugarcubes-masonry-toggle');
  if (!button) throw new Error('View switch did not attach');
  expect(button.previousElementSibling).toBe(queueSlot);
  expect(button.nextElementSibling).toBe(propertiesSlot);
  expect(button.textContent).toBe('');
  expect(button.getAttribute('aria-label')).toBe('Toggle workflow masonry');
  for (const active of [false, true, false]) {
    control.update(active, true);
    expect(button.disabled).toBe(false);
    expect(button.getAttribute('aria-pressed')).toBe(String(active));
    expect(button.title).toBe(`Switch to ${active ? 'graph' : 'masonry'} view`);
    expect(button.firstElementChild?.getAttribute('aria-hidden')).toBe('true');
    expect(button.firstElementChild?.getAttribute('style')).toContain(
      active ? 'flowchart.svg' : 'board.svg',
    );
    button.click();
  }
  expect(toggle).toHaveBeenCalledTimes(3);
  control.update(false, false);
  button.click();
  expect(toggle).toHaveBeenCalledTimes(3);
  expect(button.title).toBe('Masonry requires Nodes 2.0');
  control.dispose();
});

test('waits for the native toolbar and reuses the same button after remount without canvas fallback', async () => {
  const canvas = document.createElement('div');
  canvas.className = 'graph-canvas-panel';
  document.body.append(canvas);
  const toggle = jest.fn();
  const control = new WorkflowMasonryToolbarButton(document, toggle);
  control.setup();
  expect(document.querySelector('.sugarcubes-masonry-toggle')).toBeNull();
  const original = toolbar();
  await Promise.resolve();
  const button = original.bar.querySelector<HTMLButtonElement>('.sugarcubes-masonry-toggle');
  if (!button) throw new Error('View switch did not attach');
  original.bar.remove();
  const replacement = toolbar();
  await Promise.resolve();
  expect(replacement.bar.querySelector('.sugarcubes-masonry-toggle')).toBe(button);
  expect(button.nextElementSibling).toBe(replacement.propertiesSlot);
  control.update(false, true);
  button.click();
  expect(toggle).toHaveBeenCalledTimes(1);
  control.dispose();
  toolbar();
  await Promise.resolve();
  expect(document.querySelector('.sugarcubes-masonry-toggle')).toBeNull();
  button.click();
  expect(toggle).toHaveBeenCalledTimes(1);
});

test('waits for the queue and stays attached while Properties moves into the open panel', async () => {
  const { bar, queueSlot, propertiesSlot } = toolbar();
  queueSlot.remove();
  const control = new WorkflowMasonryToolbarButton(document, () => undefined);
  control.setup();
  expect(bar.querySelector('.sugarcubes-masonry-toggle')).toBeNull();
  propertiesSlot.remove();
  bar.append(queueSlot);
  await Promise.resolve();
  const button = bar.querySelector('.sugarcubes-masonry-toggle');
  expect(button).not.toBeNull();
  expect(button?.previousElementSibling).toBe(queueSlot);
  control.dispose();
});
