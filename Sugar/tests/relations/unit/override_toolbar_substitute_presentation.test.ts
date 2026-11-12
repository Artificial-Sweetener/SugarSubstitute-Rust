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
/** Safeguard saved floating and collapsed preferences while Substitute owns inline presentation. */
import { jest } from '@jest/globals';
import { OverrideToolbarPlacement } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarPlacement.js';
import { OverrideToolbarDisclosure } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarDisclosure.js';

afterEach(() => document.body.replaceChildren());

test('inline placement temporarily suspends floating geometry without writing user preferences', () => {
  const root = document.createElement('div');
  const grip = document.createElement('button');
  root.append(grip);
  document.body.append(root);
  const attach = jest
    .fn<(element: HTMLElement, docked: boolean) => boolean>()
    .mockReturnValue(true);
  const host = {
    attach,
    canDock: () => true,
    floatingBounds: () => null,
    dockBounds: () => null,
    dragging: () => undefined,
    preview: () => undefined,
    subscribeLayout: () => () => undefined,
    dispose: () => undefined,
  };
  const state = {
    read: () => ({ docked: false, floating: { x: 120, y: 240 }, anchor: null }),
    write: jest.fn(),
  };
  const placement = new OverrideToolbarPlacement(root, grip, host, jest.fn(), jest.fn(), state);
  placement.setInline(true);
  expect(root.dataset.placement).toBe('inline');
  expect(attach).toHaveBeenLastCalledWith(root, true);
  placement.toggleDock();
  placement.shift(10, 10);
  expect(state.write).not.toHaveBeenCalled();
  placement.setInline(false);
  expect(root.dataset.placement).toBe('floating');
  expect(attach).toHaveBeenLastCalledWith(root, false);
  expect(state.write).not.toHaveBeenCalled();
  placement.dispose();
});

test('inline disclosure presents saved collapsed fields and restores that choice on exit', () => {
  const root = document.createElement('div');
  const fields = document.createElement('div');
  const button = document.createElement('button');
  root.append(fields, button);
  document.body.append(root);
  const state = { readCollapsed: () => true, writeCollapsed: jest.fn() };
  const disclosure = new OverrideToolbarDisclosure(
    root,
    fields,
    button,
    jest.fn(),
    jest.fn(),
    state,
  );
  expect(fields.hidden).toBe(true);
  disclosure.setInline(true);
  expect(fields.hidden).toBe(false);
  expect(fields.inert).toBe(false);
  button.click();
  expect(state.writeCollapsed).not.toHaveBeenCalled();
  disclosure.setInline(false);
  expect(fields.hidden).toBe(true);
  expect(fields.inert).toBe(true);
  expect(state.writeCollapsed).not.toHaveBeenCalled();
  disclosure.dispose();
});
