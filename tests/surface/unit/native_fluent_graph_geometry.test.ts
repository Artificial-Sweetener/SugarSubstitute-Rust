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
/** Prove initial native socket reconciliation is coalesced, scoped, cancellable, and observable. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCardGraphGeometry } from '../../../frontend/comfyui/ui/surface/appearance/NativeCardGraphGeometry.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Bind native layout capabilities to an explicitly controlled presentation frame. */
function fixture() {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const cancel = jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  const synchronize = jest.fn<() => void>();
  const getSynchronizer = jest.fn<() => Promise<() => void>>().mockResolvedValue(synchronize);
  const reportError = jest.fn();
  const owner = new NativeCardGraphGeometry({ document, getSynchronizer, reportError });
  return { frames, cancel, synchronize, getSynchronizer, reportError, owner };
}

/** Create one real graph identity without mounting a projection. */
function card(id: string): HTMLElement {
  const root = document.createElement('article');
  root.className = 'lg-node';
  root.dataset.nodeId = id;
  document.body.append(root);
  return root;
}

test('coalesces newly styled graph cards and ignores repeated presentation and popup activity', async () => {
  const setup = fixture();
  const first = card('1');
  setup.owner.request(first);
  setup.owner.request(card('2'));
  expect(setup.frames).toHaveLength(1);
  expect(setup.synchronize).not.toHaveBeenCalled();
  setup.frames[0]?.(0);
  await Promise.resolve();
  expect(setup.synchronize).toHaveBeenCalledTimes(1);
  setup.owner.request(first);
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  setup.owner.request(popup);
  expect(setup.frames).toHaveLength(1);
  expect(setup.synchronize).toHaveBeenCalledTimes(1);
  setup.owner.clear();
});

test('excludes projected cards and ignores geometry identities suspended during movement', () => {
  const setup = fixture();
  const projection = document.createElement('div');
  projection.className = 'sugarcubes-cube-face__node-card';
  document.body.append(projection);
  const projected = card('1');
  projection.append(projected);
  const moving = card('2');
  moving.removeAttribute('data-node-id');
  setup.owner.request(projected);
  setup.owner.request(moving);
  expect(setup.frames).toHaveLength(0);
  setup.owner.clear();
});

test('discards a retired native capability acquisition and permits re-enabled styling', async () => {
  const setup = fixture();
  let resolve = (_synchronize: () => void): void => undefined;
  setup.getSynchronizer.mockReturnValue(
    new Promise((complete) => {
      resolve = complete;
    }),
  );
  const root = card('1');
  setup.owner.request(root);
  setup.frames[0]?.(0);
  setup.owner.clear();
  resolve(setup.synchronize);
  await Promise.resolve();
  expect(setup.synchronize).not.toHaveBeenCalled();
  setup.getSynchronizer.mockResolvedValue(setup.synchronize);
  setup.owner.request(root);
  setup.frames[1]?.(0);
  await Promise.resolve();
  expect(setup.synchronize).toHaveBeenCalledTimes(1);
  setup.owner.clear();
});

test('cancels pending frames and reports native synchronization failures with their context', async () => {
  const setup = fixture();
  setup.owner.request(card('1'));
  setup.owner.clear();
  expect(setup.cancel).toHaveBeenCalledWith(1);
  const failure = new Error('Native slots unavailable');
  setup.getSynchronizer.mockRejectedValue(failure);
  setup.owner.request(card('2'));
  setup.frames[1]?.(0);
  await Promise.resolve();
  expect(setup.reportError).toHaveBeenCalledWith(failure);
  setup.owner.clear();
});

test('resynchronizes the restored native layout once when Fluent styling is removed', async () => {
  const setup = fixture();
  setup.owner.request(card('1'));
  setup.frames[0]?.(0);
  await Promise.resolve();
  setup.owner.stylesRemoved();
  setup.frames[1]?.(0);
  await Promise.resolve();
  expect(setup.synchronize).toHaveBeenCalledTimes(2);
  setup.owner.stylesRemoved();
  expect(setup.frames).toHaveLength(2);
  setup.owner.clear();
});
