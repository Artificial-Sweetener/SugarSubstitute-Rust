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
/** Prove closed link labels surrender routes while native menus and values retain their full identity. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeLinkLabelLayout } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeLinkLabelLayout.js';
import { NativeLinkChoiceLabels } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeLinkChoiceLabels.js';

afterEach(() => {
  jest.restoreAllMocks();
});

test('compacts the route before eliding the tail and restores full text as space returns', () => {
  const frames: FrameRequestCallback[] = [];
  jest
    .spyOn(window, 'requestAnimationFrame')
    .mockImplementation((callback) => frames.push(callback));
  const flush = () => frames.splice(0).forEach((callback) => callback(0));
  const root = document.createElement('div');
  const trigger = document.createElement('button');
  trigger.dataset.testid = 'widget-select-default-trigger';
  const selected = document.createElement('span');
  selected.textContent = '🔗 pack/long-route/Cube Name';
  const option = document.createElement('div');
  option.setAttribute('role', 'option');
  option.textContent = selected.textContent;
  trigger.append(selected);
  root.append(trigger, option);
  let width = 120;
  Object.defineProperty(selected, 'clientWidth', { get: () => width });
  const labels = new NativeLinkChoiceLabels(
    root,
    new NativeLinkLabelLayout(document, jest.fn()),
    (text) => text.length * 8,
  );
  labels.update(['Independent', selected.textContent]);
  flush();
  expect(selected.dataset.scLinkDisplay).toBe('🔗 Cube Name');
  expect(selected.textContent).toBe('🔗 pack/long-route/Cube Name');
  expect(selected.title).toBe(selected.textContent);
  expect(option.textContent).toBe(selected.textContent);
  const preference = root.style.getPropertyValue('--sc-link-preferred-width');
  width = 400;
  labels.update(['Independent', selected.textContent]);
  flush();
  expect(selected.hasAttribute('data-sc-link-display')).toBe(false);
  expect(root.style.getPropertyValue('--sc-link-preferred-width')).toBe(preference);
  selected.textContent = '🔗 pack/<img src=x>';
  width = 60;
  labels.update([selected.textContent]);
  flush();
  expect(selected.dataset.scLinkDisplay).toBe('🔗 <img src=x>');
  expect(root.querySelector('img')).toBeNull();
  const writes = new MutationObserver(() => undefined);
  writes.observe(root, { attributes: true, subtree: true });
  labels.update([selected.textContent]);
  labels.update([selected.textContent]);
  expect(frames).toHaveLength(1);
  flush();
  expect(writes.takeRecords()).toHaveLength(0);
  writes.disconnect();
  labels.update([selected.textContent]);
  const cancel = jest.spyOn(window, 'cancelAnimationFrame');
  labels.dispose();
  expect(cancel).toHaveBeenCalledTimes(1);
  flush();
  expect(selected.hasAttribute('data-sc-link-display')).toBe(false);
  expect(selected.title).toBe('');
  expect(root.style.getPropertyValue('--sc-link-preferred-width')).toBe('');
});

test('reads all link geometry before writing and releases cancelled controls from the shared frame', () => {
  const frames: FrameRequestCallback[] = [];
  jest
    .spyOn(window, 'requestAnimationFrame')
    .mockImplementation((callback) => frames.push(callback));
  const cancel = jest.spyOn(window, 'cancelAnimationFrame');
  const error = jest.fn();
  const layout = new NativeLinkLabelLayout(document, error);
  const order: string[] = [];
  const first = () => {
    order.push('read a');
    return () => {
      order.push('write a');
    };
  };
  const second = () => {
    order.push('read b');
    return () => {
      order.push('write b');
    };
  };
  layout.invalidate(first);
  layout.invalidate(second);
  layout.invalidate(first);
  expect(frames).toHaveLength(1);
  frames.splice(0).forEach((frame) => frame(0));
  expect(order).toEqual(['read a', 'read b', 'write a', 'write b']);
  layout.invalidate(first);
  layout.release(first);
  expect(cancel).toHaveBeenCalledTimes(1);
  frames.splice(0).forEach((frame) => frame(0));
  expect(order).toHaveLength(4);
  const failure = new Error('Host measurement failed');
  layout.invalidate(() => {
    throw failure;
  });
  layout.invalidate(second);
  frames.splice(0).forEach((frame) => frame(0));
  expect(error).toHaveBeenCalledWith(failure);
  expect(order.slice(-2)).toEqual(['read b', 'write b']);
});
