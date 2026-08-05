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
import { createComfySettingsOverlayProps } from '../../frontend/comfyui/ui/controls/ComfySettingsSelect.js';
import { CubeAuthoringModal } from '../../frontend/comfyui/ui/dialogs/CubeAuthoringModal.js';
import {
  COMFY_SETTINGS_OVERLAY_BASE_Z_INDEX,
  DIALOG_OVERLAY_Z_INDEX,
} from '../../frontend/comfyui/ui/core/OverlayStacking.js';
import { injectDialogStyles } from '../../frontend/comfyui/ui/dialogs/DialogStyles.js';
import { TestComfySettingsSelectRenderer } from './helpers/ComfySettingsSelectTestRenderer.js';

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
  test('shows an explicit empty graph summary during an empty Cube first save', async () => {
    const modal = createModal(new TestComfySettingsSelectRenderer());
    const promise = modal.open({
      candidate: {
        defaultAlias: 'Untitled Cube',
        nodeIds: [],
        markerIds: [],
        inputCount: 0,
        outputCount: 0,
      },
    });

    expect(document.body.textContent).toContain('Selection0 nodes, 0 inputs, 0 outputs');
    const overlay = document.querySelector<HTMLElement>('.sugarcubes-create-cube-overlay')!;
    const preview = document.querySelector<HTMLElement>('.sugarcubes-create-cube__preview')!;
    const descriptionField = document
      .querySelector<HTMLTextAreaElement>('.sugarcubes-create-cube__description')!
      .closest('label')!;
    expect(overlay.style.cursor).toBe('grab');
    expect(window.getComputedStyle(overlay).alignItems).toBe('center');
    expect(
      preview.compareDocumentPosition(descriptionField) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    overlay.click();
    expect(document.body.textContent).toContain('Save SugarCube');
    document
      .querySelector<HTMLButtonElement>('.sugarcubes-create-cube-dialog .p-dialog-footer button')!
      .click();
    await expect(promise).resolves.toBeNull();
    expect(overlay.style.cursor).toBe('');
  });

  test('collects persisted metadata and renders warning-like markup literally', async () => {
    const settingsSelect = new TestComfySettingsSelectRenderer();
    const modal = createModal(settingsSelect);
    const promise = modal.open({
      candidate: {
        defaultAlias: '',
        nodeIds: [1],
        warnings: ['<img src=x onerror=alert(1)>'],
      },
      modelSuggestions: ['Flux .1 D', 'SD 1.5'],
      deriveIdentity: (name, targetModel, destination) => {
        expect(destination).toEqual({ kind: 'local' });
        return suggestPersonalCubeIdentity(name, targetModel);
      },
    });

    const form = document.querySelector<HTMLFormElement>('.sugarcubes-create-cube__form');
    expect(form).not.toBeNull();
    const inputs = form!.querySelectorAll<HTMLInputElement>('input');
    expect(inputs).toHaveLength(3);
    expect(document.body.textContent).toContain('Supported models');
    expect(form!.querySelector('datalist')).toBeNull();
    expect(document.body.textContent).toContain('1 nodes, 0 markers');
    expect(document.body.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(document.querySelector('img')).toBeNull();

    inputs[0]!.value = 'My useful cube';
    inputs[0]!.dispatchEvent(new Event('input', { bubbles: true }));
    settingsSelect.completeAutocomplete('Supported models', '');
    expect(settingsSelect.autocomplete('Supported models').suggestions).toEqual(
      expect.arrayContaining(['Flux .1 D']),
    );
    expect(settingsSelect.autocomplete('Supported models').values).toEqual(['SDXL', 'SD 1.5']);
    settingsSelect.completeAutocomplete('Supported models', 'My Bespoke Model');
    expect(settingsSelect.autocomplete('Supported models').suggestions[0]).toBe('My Bespoke Model');
    settingsSelect.selectSingle('Target model', 'Flux');
    expect(settingsSelect.single('Target model').value).toBe('Flux');
    expect(document.body.textContent).toContain('Flux/My Useful Cube');
    settingsSelect.selectSingle('Target model', 'SDXL');
    settingsSelect.enterAutocomplete('Supported models', [
      'SDXL',
      'Flux .1 D',
      'SD 1.5',
      'My Bespoke Model',
    ]);
    const description = form!.querySelector<HTMLTextAreaElement>('textarea')!;
    description.value = 'A <strong>safe</strong> Cube.';
    description.dispatchEvent(new Event('input', { bubbles: true }));
    document
      .querySelector<HTMLButtonElement>(
        '.sugarcubes-create-cube-dialog .p-dialog-footer button:last-child',
      )!
      .click();

    await expect(promise).resolves.toEqual({
      name: 'My Useful Cube',
      defaultAlias: 'SDXL/My Useful Cube',
      cubeId: 'local/personal/SDXL/My Useful Cube.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'Flux .1 D', 'SD 1.5', 'My Bespoke Model'],
      description: 'A <strong>safe</strong> Cube.',
      destination: { kind: 'local' },
    });
  });

  test('uses Another model as the explicit path for a custom target model', async () => {
    const settingsSelect = new TestComfySettingsSelectRenderer();
    const modal = createModal(settingsSelect);
    const promise = modal.open({
      candidate: { defaultAlias: 'Klein Cube', targetModel: 'SDXL' },
      modelSuggestions: ['Flux .1 D', 'Flux .1 Kontext'],
      deriveIdentity: (name, targetModel) => suggestPersonalCubeIdentity(name, targetModel),
    });

    const target = settingsSelect.single('Target model');
    const customTargetField = document.querySelector<HTMLElement>(
      '.sugarcubes-create-cube__custom-target-model',
    );
    if (!customTargetField) throw new Error('Expected the custom target-model field.');
    expect(customTargetField.hidden).toBe(true);
    expect(window.getComputedStyle(customTargetField).display).toBe('none');
    expect(target.options.map((option) => option.label)).toEqual(
      expect.arrayContaining(['Flux .1 D', 'Flux .1 Kontext', 'Another model…']),
    );
    const anotherModel = target.options.find((option) => option.label === 'Another model…');
    if (!anotherModel) throw new Error('Expected the custom target-model option.');
    settingsSelect.selectSingle('Target model', anotherModel.value);
    expect(customTargetField.hidden).toBe(false);
    expect(window.getComputedStyle(customTargetField).display).toBe('grid');

    const customTarget = document.querySelector<HTMLInputElement>(
      '.sugarcubes-create-cube__custom-target-model input',
    );
    if (!customTarget) throw new Error('Expected the custom target-model input.');
    expect(customTarget.disabled).toBe(false);
    customTarget.value = 'Flux2 Klein';
    customTarget.dispatchEvent(new Event('input', { bubbles: true }));
    expect(document.body.textContent).toContain('Flux2 Klein/Klein Cube');
    expect(settingsSelect.autocomplete('Supported models').values).toEqual(['Flux2 Klein']);

    document
      .querySelector<HTMLButtonElement>(
        '.sugarcubes-create-cube-dialog .p-dialog-footer button:last-child',
      )!
      .click();

    await expect(promise).resolves.toEqual(
      expect.objectContaining({
        targetModel: 'Flux2 Klein',
        supportedModels: ['Flux2 Klein'],
        cubeId: 'local/personal/Flux2 Klein/Klein Cube.cube',
      }),
    );
  });

  test('stacks installed Settings overlays above the modal backdrop', () => {
    expect(createComfySettingsOverlayProps()).toEqual({
      appendTo: 'body',
      autoZIndex: true,
      baseZIndex: COMFY_SETTINGS_OVERLAY_BASE_Z_INDEX,
      overlayClass: 'sugarcubes-comfy-settings-overlay',
    });
    expect(COMFY_SETTINGS_OVERLAY_BASE_Z_INDEX).toBeGreaterThan(DIALOG_OVERLAY_Z_INDEX);
    injectDialogStyles(document);
    const dialogStyles = document.getElementById('sugarcubes-dialog-styles')?.textContent;
    expect(dialogStyles).toContain('.sugarcubes-comfy-settings-overlay,');
    expect(dialogStyles).toContain('.sugarcubes-modal-overlay.is-visible ~ .p-select-overlay,');
    expect(dialogStyles).toContain(`z-index: ${COMFY_SETTINGS_OVERLAY_BASE_Z_INDEX} !important;`);
  });

  test('keeps destination selection and pack creation inside the complete save modal', async () => {
    const settingsSelect = new TestComfySettingsSelectRenderer();
    const modal = createModal(settingsSelect);
    const promise = modal.open({
      candidate: {
        defaultAlias: 'Text to Image',
        targetModel: 'SDXL',
        nodeIds: [1, 2],
        markerIds: [3, 4],
        inputCount: 1,
        outputCount: 1,
      },
      destinations: [
        {
          key: 'local/personal',
          label: 'Personal cubes',
          detail: 'Saved locally',
          destination: { kind: 'local' },
        },
        {
          key: 'pack/artist/cubes',
          label: 'cubes',
          detail: 'artist',
          destination: {
            kind: 'pack',
            owner: 'artist',
            repo: 'cubes',
            repoRef: 'artist/cubes',
          },
        },
        {
          key: 'create-pack',
          label: 'Create a new Cube Pack…',
          action: 'create-pack',
        },
      ],
      onCreateDestination: async () => ({
        key: 'pack/artist/new-cubes',
        label: 'new-cubes',
        detail: 'artist',
        destination: {
          kind: 'pack',
          owner: 'artist',
          repo: 'new-cubes',
          repoRef: 'artist/new-cubes',
        },
      }),
      deriveIdentity: (name, targetModel, destination) =>
        destination.kind === 'local'
          ? suggestPersonalCubeIdentity(name, targetModel)
          : {
              name,
              defaultAlias: `${targetModel}/${name}`,
              cubeId: `${destination.repoRef}/${targetModel}/${name}.cube`,
            },
    });
    settingsSelect.selectSingle('Save to', 'pack/artist/cubes');
    expect(document.body.textContent).toContain('artist/cubes/SDXL/Text to Image.cube');
    expect(document.body.textContent).toContain('2 nodes, 1 inputs, 1 outputs');

    settingsSelect.selectSingle('Save to', 'create-pack');
    await Promise.resolve();
    expect(settingsSelect.single('Save to').value).toBe('pack/artist/new-cubes');
    expect(document.body.textContent).toContain('artist/new-cubes/SDXL/Text to Image.cube');

    document
      .querySelector<HTMLButtonElement>(
        '.sugarcubes-create-cube-dialog .p-dialog-footer button:last-child',
      )!
      .click();
    await expect(promise).resolves.toEqual(
      expect.objectContaining({
        cubeId: 'artist/new-cubes/SDXL/Text to Image.cube',
        destination: expect.objectContaining({ repoRef: 'artist/new-cubes' }),
      }),
    );
  });

  test('confirms an existing Cube without pretending it is a new selection or destination', async () => {
    const settingsSelect = new TestComfySettingsSelectRenderer();
    const modal = createModal(settingsSelect);
    const deriveIdentity = jest.fn((name: string) => ({
      name,
      defaultAlias: `SDXL/${name}`,
      cubeId: 'local/personal/SDXL/Existing.cube',
    }));
    const promise = modal.open({
      candidate: {
        cubeId: 'local/personal/SDXL/Existing.cube',
        defaultAlias: 'SDXL/Existing',
        targetModel: 'SDXL',
        supportedModels: ['SDXL', 'Flux'],
      },
      destinationLocked: true,
      deriveIdentity,
    });

    expect(document.body.textContent).not.toContain('selected nodes');
    expect(document.body.textContent).toContain('local/personal/SDXL/Existing.cube');
    expect(document.body.textContent).toContain('Review the Cube metadata before saving changes.');
    expect(document.body.textContent).not.toContain('before its first save');
    expect(settingsSelect.autocomplete('Supported models').values).toEqual(['SDXL', 'Flux']);
    expect(settingsSelect.single('Save to').disabled).toBe(true);
    expect(document.querySelector('.sugarcubes-create-cube__form select')).toBeNull();
    document
      .querySelector<HTMLButtonElement>(
        '.sugarcubes-create-cube-dialog .p-dialog-footer button:last-child',
      )!
      .click();

    await expect(promise).resolves.toEqual(
      expect.objectContaining({
        cubeId: 'local/personal/SDXL/Existing.cube',
        defaultAlias: 'SDXL/Existing',
      }),
    );
  });
});

/** Create the modal with a deterministic native Settings component boundary. */
function createModal(settingsSelectRenderer: TestComfySettingsSelectRenderer): CubeAuthoringModal {
  return new CubeAuthoringModal({
    adapter: {
      getDocument: () => document,
      getWindow: () => window,
    },
    settingsSelectRenderer,
  });
}
