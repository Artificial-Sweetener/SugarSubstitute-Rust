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
/** Verify the name-only personal cube entry point. */

import { beforeEach, describe, expect, test } from '@jest/globals';
import {
  buildPersonalCubeId,
  suggestPersonalCubeIdentity,
} from '../../web/comfyui/ui/create/PersonalCubeIdentity.js';
import { CreatePersonalCubeModal } from '../../web/comfyui/ui/dialogs/CreatePersonalCubeModal.js';

beforeEach(() => {
  document.body.replaceChildren();
});

describe('personal cube identity', () => {
  test('builds flat collision-safe local identities without pack metadata', () => {
    expect(buildPersonalCubeId('Text to Image')).toBe('local/personal/Text to Image.cube');
    expect(
      suggestPersonalCubeIdentity('text to image', [
        'local/personal/Text to Image.cube',
        'local/personal/Text to Image 2.cube',
      ]),
    ).toEqual({
      name: 'Text to Image 3',
      defaultAlias: 'Text to Image 3',
      cubeId: 'local/personal/Text to Image 3.cube',
    });
  });
});

describe('personal cube modal', () => {
  test('asks only for a name and renders warning-like markup literally', async () => {
    const modal = new CreatePersonalCubeModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const promise = modal.open({
      candidate: {
        defaultAlias: '',
        nodeIds: [1],
        markerIds: [2, 3],
        warnings: ['<img src=x onerror=alert(1)>'],
      },
      deriveIdentity: (name) => suggestPersonalCubeIdentity(name),
    });

    const form = document.querySelector<HTMLFormElement>('.sugarcubes-create-personal__form');
    expect(form).not.toBeNull();
    const inputs = form!.querySelectorAll<HTMLInputElement>('input');
    expect(inputs).toHaveLength(1);
    expect(document.body.textContent).toContain('Saved privately');
    expect(document.body.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(document.querySelector('img')).toBeNull();

    inputs[0]!.value = 'My useful cube';
    inputs[0]!.dispatchEvent(new Event('input', { bubbles: true }));
    document
      .querySelector<HTMLButtonElement>('.sugarcubes-create-personal-dialog button:last-child')!
      .click();

    await expect(promise).resolves.toEqual({
      name: 'My Useful Cube',
      defaultAlias: 'My Useful Cube',
      cubeId: 'local/personal/My Useful Cube.cube',
    });
  });
});
