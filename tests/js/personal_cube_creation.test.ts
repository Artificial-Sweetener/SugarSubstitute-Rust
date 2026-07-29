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
/** Verify full personal Cube authoring before its first save. */

import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import {
  buildPersonalCubeId,
  suggestPersonalCubeIdentity,
} from '../../frontend/comfyui/ui/create/PersonalCubeIdentity.js';
import { CubeAuthoringModal } from '../../frontend/comfyui/ui/dialogs/CubeAuthoringModal.js';

beforeEach(() => {
  document.body.replaceChildren();
});

describe('personal cube identity', () => {
  test('builds collision-safe local identities under the selected model family', () => {
    expect(buildPersonalCubeId('Text to Image', 'SDXL')).toBe(
      'local/personal/SDXL/Text to Image.cube',
    );
    expect(
      suggestPersonalCubeIdentity('text to image', 'SDXL', [
        'local/personal/SDXL/Text to Image.cube',
        'local/personal/SDXL/Text to Image 2.cube',
      ]),
    ).toEqual({
      name: 'Text to Image 3',
      defaultAlias: 'SDXL/Text to Image 3',
      cubeId: 'local/personal/SDXL/Text to Image 3.cube',
    });
  });
});

describe('Cube authoring modal', () => {
  test('collects persisted metadata and renders warning-like markup literally', async () => {
    const modal = new CubeAuthoringModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const promise = modal.open({
      candidate: {
        defaultAlias: '',
        nodeIds: [1],
        warnings: ['<img src=x onerror=alert(1)>'],
      },
      modelSuggestions: ['Flux .1 D'],
      deriveIdentity: async (name, targetModel, destination) => {
        expect(destination).toEqual({ kind: 'local' });
        return suggestPersonalCubeIdentity(name, targetModel);
      },
    });

    const form = document.querySelector<HTMLFormElement>('.sugarcubes-create-cube__form');
    expect(form).not.toBeNull();
    const inputs = form!.querySelectorAll<HTMLInputElement>('input');
    expect(inputs).toHaveLength(3);
    expect(document.body.textContent).toContain('Supported models');
    expect(
      form!.querySelector<HTMLDataListElement>('#sugarcubes-cube-authoring-model-suggestions')
        ?.textContent,
    ).toContain('Flux .1 D');
    expect(document.body.textContent).toContain('1 selected nodes');
    expect(document.body.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(document.querySelector('img')).toBeNull();

    inputs[0]!.value = 'My useful cube';
    inputs[0]!.dispatchEvent(new Event('input', { bubbles: true }));
    const description = form!.querySelector<HTMLTextAreaElement>('textarea')!;
    description.value = 'A <strong>safe</strong> Cube.';
    description.dispatchEvent(new Event('input', { bubbles: true }));
    document
      .querySelector<HTMLButtonElement>('.sugarcubes-create-cube-dialog button:last-child')!
      .click();

    await expect(promise).resolves.toEqual({
      name: 'My Useful Cube',
      defaultAlias: 'SDXL/My Useful Cube',
      cubeId: 'local/personal/SDXL/My Useful Cube.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: 'A <strong>safe</strong> Cube.',
      destination: { kind: 'local' },
    });
  });

  test('confirms an existing Cube without pretending it is a new selection or destination', async () => {
    const modal = new CubeAuthoringModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const deriveIdentity = jest.fn(async (name: string) => ({
      name,
      defaultAlias: `SDXL/${name}`,
      cubeId: 'local/personal/SDXL/Existing.cube',
    }));
    const promise = modal.open({
      candidate: {
        cubeId: 'local/personal/SDXL/Existing.cube',
        defaultAlias: 'SDXL/Existing',
        targetModel: 'SDXL',
        supportedModels: ['SDXL'],
      },
      destinationLocked: true,
      deriveIdentity,
    });

    expect(document.body.textContent).not.toContain('selected nodes');
    expect(document.body.textContent).toContain('local/personal/SDXL/Existing.cube');
    expect(document.body.textContent).toContain('Review the Cube metadata before saving changes.');
    expect(document.body.textContent).not.toContain('before its first save');
    expect(
      document.querySelectorAll<HTMLSelectElement>('.sugarcubes-create-cube__form select')[1]
        ?.disabled,
    ).toBe(true);
    document
      .querySelector<HTMLButtonElement>('.sugarcubes-create-cube-dialog button:last-child')!
      .click();

    await expect(promise).resolves.toEqual(
      expect.objectContaining({
        cubeId: 'local/personal/SDXL/Existing.cube',
        defaultAlias: 'SDXL/Existing',
      }),
    );
  });
});
