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
/** Safeguard authoring backdrop navigation while distinguishing a dismissal click from a pan. */
import { expect, jest, test } from '@jest/globals';
import { ModalShell } from '../../../frontend/comfyui/ui/dialogs/ModalShell.js';
import { CreateModalGraphNavigator } from '../../../frontend/comfyui/ui/dialogs/CreateModalGraphNavigator.js';

test('backdrop drag and wheel change only the viewport and preserve the modal', async () => {
  const canvas = {
    ds: { scale: 2, offset: [10, 20], changeScale: jest.fn() },
    setDirty: jest.fn(),
  };
  const adapter = { getDocument: () => document, getWindow: () => window, getCanvas: () => canvas };
  const shell = new ModalShell({ adapter });
  const navigator = new CreateModalGraphNavigator({ adapter });
  const result = shell.open({ allowOverlayClose: false });
  const overlay = shell.elements.overlay;
  if (!overlay) throw new Error('Missing authoring backdrop');
  navigator.attach(overlay, shell.elements.dialog);
  try {
    overlay.dispatchEvent(
      new MouseEvent('pointerdown', {
        clientX: 100,
        clientY: 100,
        bubbles: true,
        cancelable: true,
      }),
    );
    overlay.dispatchEvent(
      new MouseEvent('pointermove', {
        clientX: 140,
        clientY: 120,
        bubbles: true,
        cancelable: true,
      }),
    );
    overlay.dispatchEvent(
      new MouseEvent('pointerup', { clientX: 140, clientY: 120, bubbles: true, cancelable: true }),
    );
    overlay.dispatchEvent(
      new WheelEvent('wheel', {
        deltaY: -10,
        clientX: 100,
        clientY: 100,
        bubbles: true,
        cancelable: true,
      }),
    );
    expect(canvas.ds.offset).toEqual([30, 30]);
    expect(canvas.ds.changeScale).toHaveBeenCalledWith(2.2, [100, 100], false);
    expect(overlay.classList.contains('is-visible')).toBe(true);
  } finally {
    navigator.detach();
    shell.close(null);
    await result;
    overlay.remove();
  }
});

test('drag-generated click stays open and a subsequent ordinary backdrop click cancels', async () => {
  const adapter = { getDocument: () => document, getWindow: () => window };
  const shell = new ModalShell({ adapter });
  const navigator = new CreateModalGraphNavigator({ adapter });
  const result = shell.open();
  const overlay = shell.elements.overlay;
  if (!overlay) throw new Error('Missing authoring backdrop');
  navigator.attach(overlay, shell.elements.dialog);
  try {
    overlay.dispatchEvent(
      new MouseEvent('pointerdown', { clientX: 10, clientY: 10, bubbles: true, cancelable: true }),
    );
    overlay.dispatchEvent(
      new MouseEvent('pointermove', { clientX: 30, clientY: 10, bubbles: true, cancelable: true }),
    );
    overlay.dispatchEvent(
      new MouseEvent('pointerup', { clientX: 30, clientY: 10, bubbles: true, cancelable: true }),
    );
    overlay.click();
    expect(overlay.classList.contains('is-visible')).toBe(true);
    overlay.dispatchEvent(
      new MouseEvent('pointerdown', { clientX: 30, clientY: 10, bubbles: true, cancelable: true }),
    );
    overlay.dispatchEvent(
      new MouseEvent('pointerup', { clientX: 30, clientY: 10, bubbles: true, cancelable: true }),
    );
    overlay.click();
    expect(overlay.classList.contains('is-visible')).toBe(false);
    await expect(result).resolves.toBeNull();
  } finally {
    navigator.detach();
    shell.close(null);
    overlay.remove();
  }
});
