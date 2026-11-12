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
/** Verify exclusive card relocation, original geometry, and source lifecycle cleanup. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  NativeCardPresentationOwner,
  prepareNativeCardPresentation,
  measureNativeCardPresentationSources,
  restoreNativeCardPresentations,
} from '../../../frontend/comfyui/ui/surface/NativeCardPresentationOwner.js';
import { CubePromptTextareaAutosizer } from '../../../frontend/comfyui/ui/surface/CubePromptTextareaAutosizer.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('moves the original controls with listeners and reserves their exact source geometry', () => {
  const { owner, source, destination, cell, input, resumed } = fixture();
  const listener = jest.fn();
  input.addEventListener('input', listener);
  input.focus();
  input.setSelectionRange(1, 3);
  const invalidated = jest.fn();
  const transfer = prepareNativeCardPresentation(cell, invalidated)!;
  expect(owner.borrowed).toBe(true);
  expect(prepareNativeCardPresentation(cell, jest.fn())).toBeNull();
  transfer.move(destination);
  expect(destination.querySelector('input')).toBe(input);
  expect(document.activeElement).toBe(input);
  expect(input.selectionStart).toBe(1);
  expect(input.selectionEnd).toBe(3);
  input.dispatchEvent(new Event('input'));
  expect(listener).toHaveBeenCalledTimes(1);
  const origin = source.firstElementChild as HTMLElement;
  expect(origin.dataset.cubeNodeId).toBe('7');
  expect(origin.classList.contains('sugarcubes-cube-face__node-card')).toBe(false);
  expect(origin.style.height).toBe('123.5px');
  expect(origin.style.left).toBe('42px');
  expect(origin.style.width).toBe('321px');
  cell.style.color = 'red';
  transfer.restore();
  transfer.restore();
  expect(source.firstElementChild).toBe(cell);
  expect(cell.style.left).toBe('42px');
  expect(cell.style.top).toBe('75px');
  expect(cell.style.width).toBe('321px');
  expect(cell.style.position).toBe('absolute');
  expect(cell.style.color).toBe('red');
  expect(destination.children).toHaveLength(0);
  expect(owner.borrowed).toBe(false);
  expect(resumed).toHaveBeenCalledTimes(1);
  expect(invalidated).not.toHaveBeenCalled();
  owner.dispose();
});

test('returns every loan before source teardown and invalidates each borrower once', () => {
  const { owner, source, destination, cell, resumed } = fixture();
  const invalidated = jest.fn(() => expect(cell.parentElement).toBe(source));
  const transfer = prepareNativeCardPresentation(cell, invalidated)!;
  transfer.move(destination);
  owner.reset([]);
  expect(transfer.active).toBe(false);
  expect(invalidated).toHaveBeenCalledTimes(1);
  expect(resumed).not.toHaveBeenCalled();
  expect(prepareNativeCardPresentation(cell, jest.fn())).toBeNull();
  owner.dispose();
  transfer.restore();
  expect(invalidated).toHaveBeenCalledTimes(1);
});

test('reads all prepared source heights before moving any card', () => {
  const { owner, source, destination, cell } = fixture();
  const second = cell.cloneNode(true) as HTMLElement;
  source.append(second);
  owner.reset([cell, second]);
  const computedStyle = window.getComputedStyle.bind(window);
  const reads: number[] = [];
  jest.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) => {
    if (element === cell || element === second) reads.push(destination.children.length);
    return computedStyle(element, pseudo);
  });
  const firstTransfer = prepareNativeCardPresentation(cell, jest.fn())!;
  const secondTransfer = prepareNativeCardPresentation(second, jest.fn())!;
  firstTransfer.move(destination);
  secondTransfer.move(destination);
  expect(reads).toEqual([0, 0]);
  firstTransfer.restore();
  expect(owner.borrowed).toBe(true);
  secondTransfer.restore();
  expect([...source.children]).toEqual([cell, second]);
  owner.dispose();
});

test('settles prompt height at its restored source width before resuming source layout', () => {
  const { owner, cell, destination } = fixture();
  const textarea = document.createElement('textarea');
  Object.defineProperty(textarea, 'scrollHeight', {
    get: () => (cell.style.width === '321px' ? 80 : 160),
  });
  cell.append(textarea);
  const autosizer = new CubePromptTextareaAutosizer(document);
  autosizer.bind(textarea);
  const transfer = prepareNativeCardPresentation(cell, jest.fn())!;
  transfer.move(destination);
  textarea.dispatchEvent(new Event('input'));
  expect(textarea.style.height).toBe('160px');
  transfer.restore();
  expect(textarea.style.height).toBe('80px');
  autosizer.dispose();
  owner.dispose();
});

test('returns a presentation in geometry-write, prompt-read, then source-layout batches', () => {
  const first = fixture(),
    second = fixture();
  const autosizer = new CubePromptTextareaAutosizer(document);
  const reads: number[] = [];
  const transfers = [first, second].map(({ cell, destination }) => {
    const textarea = document.createElement('textarea');
    Object.defineProperty(textarea, 'scrollHeight', {
      get: () => {
        reads.push([first, second].filter(({ cell }) => cell.style.width === '321px').length);
        return cell.style.width === '321px' ? 80 : 160;
      },
    });
    cell.append(textarea);
    autosizer.bind(textarea);
    const transfer = prepareNativeCardPresentation(cell, jest.fn())!;
    transfer.move(destination);
    return transfer;
  });
  reads.length = 0;
  first.resumed.mockImplementation(() => {
    expect(second.cell.querySelector('textarea')?.style.height).toBe('80px');
  });
  restoreNativeCardPresentations([first.cell, second.cell]);
  expect(reads).toEqual([2, 2]);
  expect(transfers.every((transfer) => !transfer.active)).toBe(true);
  for (const { owner, resumed } of [first, second]) {
    expect(owner.borrowed).toBe(false);
    expect(resumed).toHaveBeenCalledTimes(1);
    owner.dispose();
  }
  autosizer.dispose();
});

test('uses the host state-preserving move capability for connected native cards in both directions', () => {
  const { owner, source, destination, cell } = fixture();
  const moving = jest.fn((target: HTMLElement, node: Node, before: Node | null) =>
    target.insertBefore(node, before),
  );
  for (const target of [source, destination])
    Object.defineProperty(target, 'moveBefore', {
      value: (node: Node, before: Node | null) => moving(target, node, before),
    });
  const transfer = prepareNativeCardPresentation(cell, jest.fn())!;
  transfer.move(destination);
  transfer.restore();
  expect(moving.mock.calls.map(([target]) => target)).toEqual([destination, source]);
  owner.dispose();
});

/** Build a native card whose rendered geometry is independent of graph zoom. */
function fixture() {
  const source = document.createElement('div');
  const destination = document.createElement('article');
  document.body.append(source, destination);
  const cell = document.createElement('div');
  cell.className = 'sugarcubes-cube-face__node-card';
  cell.dataset.cubeNodeId = '7';
  cell.style.cssText = 'position:absolute;left:42px;top:75px;width:321px;height:123.5px';
  const input = document.createElement('input');
  input.value = 'native';
  cell.append(input);
  source.append(cell);
  const resumed = jest.fn();
  const owner = new NativeCardPresentationOwner(resumed);
  owner.reset([cell]);
  return { owner, source, destination, cell, input, resumed };
}

test('measures identified sources before new presentation writes and reuses that measurement for loans', () => {
  const { owner, cell, destination } = fixture();
  const computed = jest.spyOn(window, 'getComputedStyle');
  measureNativeCardPresentationSources([cell, cell]);
  expect(computed).toHaveBeenCalledTimes(1);
  destination.append(document.createElement('header'));
  const transfer = prepareNativeCardPresentation(cell, jest.fn())!;
  expect(computed).toHaveBeenCalledTimes(1);
  transfer.move(destination);
  transfer.restore();
  computed.mockClear();
  measureNativeCardPresentationSources([cell]);
  expect(computed).toHaveBeenCalledTimes(1);
  owner.dispose();
});
