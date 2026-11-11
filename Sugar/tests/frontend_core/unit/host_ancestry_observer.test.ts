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
/** Verify identified host ancestry observation excludes unrelated descendant mutations. */

import { jest } from '@jest/globals';
import { HostAncestryObserver } from '../../../frontend/comfyui/ui/core/HostAncestryObserver.js';

test('observes only the supplied nodes and releases their callbacks', async () => {
  const parent = document.createElement('div');
  const surface = document.createElement('div');
  const sibling = document.createElement('div');
  parent.append(surface, sibling);
  document.body.append(parent);
  const changed = jest.fn();
  const observer = new HostAncestryObserver(window, changed);
  try {
    observer.observe(surface, [surface, parent], []);
    sibling.style.color = 'red';
    surface.append(document.createElement('span'));
    await Promise.resolve();
    expect(changed).not.toHaveBeenCalled();
    parent.style.visibility = 'hidden';
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(1);
    parent.append(document.createElement('div'));
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(2);
    observer.disconnect();
    parent.style.visibility = 'visible';
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(2);
  } finally {
    observer.disconnect();
    parent.remove();
  }
});

test('accepts explicitly owned attributes without observing unrelated descendants', async () => {
  const parent = document.createElement('div');
  const surface = document.createElement('div');
  const sibling = document.createElement('div');
  parent.append(surface, sibling);
  document.body.append(parent);
  const changed = jest.fn();
  const observer = new HostAncestryObserver(window, changed, ['data-canvas-motion']);
  try {
    observer.observe(surface, [surface, parent], []);
    sibling.dataset.canvasMotion = 'enter';
    parent.dataset.unrelated = 'ignored';
    await Promise.resolve();
    expect(changed).not.toHaveBeenCalled();
    parent.dataset.canvasMotion = 'exit';
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(1);
    observer.disconnect();
    delete parent.dataset.canvasMotion;
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(1);
  } finally {
    observer.disconnect();
    parent.remove();
  }
});

test('attribute-only material observation ignores sibling mounts and retires with the owner', async () => {
  const parent = document.createElement('section');
  const surface = document.createElement('div');
  parent.append(surface);
  document.body.append(parent);
  const changed = jest.fn();
  const observer = new HostAncestryObserver(window, changed, ['data-sc-color-scheme']);
  try {
    observer.observeAttributes([surface, parent, document.body]);
    const popup = document.createElement('aside');
    document.body.append(popup);
    parent.append(document.createElement('span'));
    await Promise.resolve();
    expect(changed).not.toHaveBeenCalled();
    parent.dataset.scColorScheme = 'light';
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(1);
    observer.disconnect();
    parent.dataset.scColorScheme = 'dark';
    await Promise.resolve();
    expect(changed).toHaveBeenCalledTimes(1);
    popup.remove();
  } finally {
    observer.disconnect();
    parent.remove();
  }
});
