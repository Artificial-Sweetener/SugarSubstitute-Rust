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
/** Prove portal changes never promote their parent into unrelated surface work. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { HostSurfaceObserver } from '../../../frontend/comfyui/ui/core/HostSurfaceObserver.js';

afterEach(() => document.body.replaceChildren());

test('reports identified mounts and local content, excludes owned descendants, and stops after disposal', async () => {
  const changed = jest.fn();
  const observer = new HostSurfaceObserver(document.body, '.card', changed, {
    content: true,
    exclude: '.owned',
  });
  const card = document.createElement('div');
  card.className = 'card';
  const popup = document.createElement('div');
  document.body.append(card);
  await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'mounted' }]);
  changed.mockClear();
  document.body.append(popup);
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  card.append(document.createElement('input'));
  await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'content' }]);
  const owned = document.createElement('div');
  owned.className = 'owned';
  card.append(owned);
  await Promise.resolve();
  changed.mockClear();
  owned.append(document.createElement('span'));
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  card.remove();
  await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'removed' }]);
  observer.disconnect();
  changed.mockClear();
  document.body.append(card);
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
});

test('flushes queued host mounts once and discards only subsequent owned mutations', async () => {
  const changed = jest.fn();
  const observer = new HostSurfaceObserver(document.body, '.card', changed, { content: true });
  const card = document.createElement('div');
  card.className = 'card';
  document.body.append(card);
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'mounted' }]);
  changed.mockClear();
  card.append(document.createElement('input'));
  observer.discardPending();
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  card.append(document.createElement('button'));
  await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'content' }]);
  observer.disconnect();
});

test('identifies relocation separately from native content changes in the same delivery', () => {
  const changed = jest.fn();
  const source = document.createElement('section');
  const destination = document.createElement('section');
  const card = document.createElement('div');
  card.className = 'card';
  source.append(card);
  document.body.append(source, destination);
  const observer = new HostSurfaceObserver(document.body, '.card', changed, { content: true });
  destination.append(card);
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'mounted', relocated: true }]);
  card.append(document.createElement('input'));
  source.append(card);
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'content', relocated: true }]);
  observer.disconnect();
});

test('keeps opaque editor paint out of host content observation while retaining card lifecycles', () => {
  const changed = jest.fn();
  const observer = new HostSurfaceObserver(document.body, '.card', changed, {
    content: true,
    characterData: true,
    attributes: ['class'],
  });
  const card = document.createElement('section');
  card.className = 'card';
  const editor = document.createElement('div');
  editor.setAttribute('data-sc-host-opaque', '');
  card.append(editor);
  document.body.append(card);
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'mounted' }]);
  changed.mockClear();
  const line = document.createElement('div');
  editor.append(line);
  const text = document.createTextNode('typed');
  line.append(text);
  text.data = 'typed text';
  line.className = 'rich-decoration';
  editor.replaceChildren(document.createElement('span'));
  observer.flushPending();
  expect(changed).not.toHaveBeenCalled();
  card.append(document.createElement('textarea'));
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'content' }]);
  changed.mockClear();
  card.remove();
  observer.flushPending();
  expect(changed).toHaveBeenLastCalledWith([{ element: card, kind: 'removed' }]);
  observer.disconnect();
});
