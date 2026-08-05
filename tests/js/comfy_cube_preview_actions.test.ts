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
/** Verify Cube output images retain Comfy's native action contract. */

import { describe, expect, jest, test } from '@jest/globals';

import {
  ComfyCubePreviewActions,
  type ComfyPreviewMenuOption,
} from '../../frontend/comfyui/ui/surface/ComfyCubePreviewActions.js';

describe('ComfyCubePreviewActions', () => {
  test('exposes native open, copy, and save image actions without preview transforms', async () => {
    document.body.replaceChildren();
    let menu: readonly ComfyPreviewMenuOption[] = [];
    const openWindow = jest.fn();
    const writeClipboard = jest.fn(async () => undefined);
    const fetchBlob = jest.fn(async () => new Blob(['image'], { type: 'image/png' }));
    const click = jest
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => undefined);
    const actions = new ComfyCubePreviewActions({
      document,
      openWindow,
      writeClipboard,
      fetchBlob,
      showContextMenu: (options) => {
        menu = options;
      },
      reportError: jest.fn(),
    });
    const event = new MouseEvent('contextmenu', { cancelable: true });
    const item = {
      key: 'image',
      url: '/api/view?filename=proof.png&type=temp&preview=webp',
      label: 'image',
    };

    actions.openContextMenu(item, event);

    expect(event.defaultPrevented).toBe(true);
    expect(menu.map((option) => option.content)).toEqual([
      'Open Image',
      'Copy Image',
      'Save Image',
    ]);
    await menu[0]?.callback();
    await menu[1]?.callback();
    await menu[2]?.callback();
    expect(openWindow).toHaveBeenCalledWith(
      'http://localhost/api/view?filename=proof.png&type=temp',
    );
    expect(fetchBlob).toHaveBeenCalledWith(
      'http://localhost/api/view?filename=proof.png&type=temp',
    );
    expect(writeClipboard).toHaveBeenCalledWith(expect.any(Blob), expect.any(HTMLImageElement));
    expect(click).toHaveBeenCalledTimes(1);
    const anchor = document.querySelector<HTMLAnchorElement>('a[download="proof.png"]');
    expect(anchor?.href).toBe('http://localhost/api/view?filename=proof.png&type=temp');
    click.mockRestore();
  });

  test('uses the same source download for the hover action', () => {
    document.body.replaceChildren();
    const click = jest
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => undefined);
    const actions = new ComfyCubePreviewActions({
      document,
      openWindow: jest.fn(),
      writeClipboard: jest.fn(async () => undefined),
      fetchBlob: jest.fn(async () => new Blob()),
      showContextMenu: jest.fn(),
      reportError: jest.fn(),
    });

    actions.download({
      key: 'image',
      url: '/api/view?filename=proof.png&preview=jpeg',
      label: 'image',
    });

    expect(click).toHaveBeenCalledTimes(1);
    expect(document.querySelector<HTMLAnchorElement>('a')?.href).toBe(
      'http://localhost/api/view?filename=proof.png',
    );
    click.mockRestore();
  });
});
