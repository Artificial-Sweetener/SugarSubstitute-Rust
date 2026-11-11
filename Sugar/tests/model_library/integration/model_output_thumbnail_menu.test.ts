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
/** Prove nested output actions, keyboard navigation and literal labels preserve workflow values. */
import { createApp, h, nextTick } from 'vue';
import { jest } from '@jest/globals';
import { ModelContextMenuView } from '../../../frontend/comfyui/ui/model_library/vue/ModelContextMenuView.js';
import { modelOutputThumbnailMenu } from '../../../frontend/comfyui/ui/model_library/ModelOutputThumbnailMenu.js';
import type { ModelContextEntry } from '../../../frontend/comfyui/ui/model_library/ModelContextMenu.js';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import type { ModelOutputThumbnails } from '../../../frontend/comfyui/ui/model_library/ModelOutputThumbnails.js';

/** Exercise the same submenu emitted by the reference hierarchy owner. */
test('arrows enter and leave submenus, hover does not steal focus, and labels render literally', async () => {
  const root = document.createElement('div');
  document.body.append(root);
  const action = jest.fn<(entry: ModelContextEntry) => void>();
  const menu = modelOutputThumbnailMenu(
    [
      {
        id: 'one',
        sceneKey: '',
        sceneTitle: '',
        sceneOrder: 0,
        sourceKey: 'result',
        sourceLabel: '<img src=x>',
        batch: 1,
      },
      {
        id: 'two',
        sceneKey: '',
        sceneTitle: '',
        sceneOrder: 0,
        sourceKey: 'result',
        sourceLabel: '<img src=x>',
        batch: 1,
      },
    ],
    null,
  );
  const app = createApp({
    render: () => h(ModelContextMenuView, { entries: [menu], x: 200, y: 100, onAction: action }),
  });
  app.mount(root);
  try {
    await nextTick();
    const parent = document.querySelector<HTMLButtonElement>('[aria-haspopup=menu]');
    if (!parent) throw new Error('Missing submenu trigger.');
    parent.dispatchEvent(new MouseEvent('pointerenter', { bubbles: false }));
    await nextTick();
    expect(document.activeElement).toBe(parent);
    parent.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await nextTick();
    expect(document.activeElement?.textContent).toContain('<img src=x>');
    expect(document.querySelector('img')).toBeNull();
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }),
    );
    await nextTick();
    expect(document.activeElement?.textContent).toBe('Image 1');
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }),
    );
    expect(document.activeElement?.textContent).toBe('Image 2');
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }),
    );
    expect(action).not.toHaveBeenCalled();
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }),
    );
    await nextTick();
    expect(document.activeElement?.textContent).toContain('<img src=x>');
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }),
    );
    await nextTick();
    const image = document.activeElement;
    if (!(image instanceof HTMLButtonElement)) throw new Error('Missing concrete image action.');
    image.click();
    expect(action).toHaveBeenCalledWith(expect.objectContaining({ imageId: 'one' }));
  } finally {
    app.unmount();
    root.remove();
  }
});

test.each([false, true])(
  'thumbnail assignment preserves native model values and reports upload failure=%s',
  async (fail) => {
    const root = document.createElement('div');
    document.body.append(root);
    const value = 'native.safetensors';
    const base = modelPickerChoices([], 'checkpoints', [value])[0];
    if (!base) throw new Error('Missing fixture model.');
    const writes = jest.fn();
    const feedback = jest.fn();
    const upload = jest.fn<ModelPickerActions['replaceThumbnail']>(async () => {
      if (fail) throw new Error('Upload unavailable');
    });
    const actions: ModelPickerActions = {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      classifyContent: async () => undefined,
      refreshMetadata: async () => undefined,
      replaceThumbnail: upload,
      releases: async () => [],
      acquire: async () => '21.safetensors',
    };
    const image = new File(['fixture'], 'output.png', { type: 'image/png' });
    const picker = mountModelPicker(
      root,
      {
        label: 'ckpt_name',
        kind: 'checkpoints',
        options: [value],
        anchor: root,
        readValue: () => value,
        writeValue: writes,
        disabled: () => false,
        subscribe: () => () => undefined,
      },
      [{ ...base, id: 'a'.repeat(64) }],
      actions,
      feedback,
      null,
      null,
      {
        choices: () => [
          {
            id: 'image',
            sceneKey: '',
            sceneTitle: '',
            sceneOrder: 0,
            sourceKey: 'output',
            sourceLabel: 'Result',
            batch: 1,
          },
        ],
        activeChoice: () => null,
        image: async () => image,
      },
    );
    const consoleError = jest.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      root
        .querySelector('input')
        ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
      await nextTick();
      const command = [...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')].find(
        (element) => element.textContent === 'Set thumbnail from canvas',
      );
      if (!command) throw new Error('Missing thumbnail command.');
      command.click();
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
      expect(upload).toHaveBeenCalledWith('a'.repeat(64), image, expect.any(AbortSignal));
      expect(writes).not.toHaveBeenCalled();
      expect(feedback).toHaveBeenCalledWith(
        fail ? 'error' : 'info',
        fail ? 'Upload unavailable' : 'Model thumbnail updated.',
      );
    } finally {
      picker.dispose();
      root.remove();
      consoleError.mockRestore();
    }
  },
);

test('does not upload a late output image after its native model field unmounts', async () => {
  const root = document.createElement('div');
  document.body.append(root);
  const value = 'native.safetensors';
  const item = modelPickerChoices([], 'checkpoints', [value])[0];
  if (!item) throw new Error('Missing native fixture model.');
  let finishImage: (image: File) => void = () => {
    throw new Error('Missing pending output image.');
  };
  const pending = new Promise<File>((resolve) => {
    finishImage = resolve;
  });
  const readImage = jest.fn<ModelOutputThumbnails['image']>(async () => pending);
  const upload = jest.fn<ModelPickerActions['replaceThumbnail']>(async () => undefined);
  const feedback = jest.fn();
  const writes = jest.fn();
  const picker = mountModelPicker(
    root,
    {
      label: 'ckpt_name',
      kind: 'checkpoints',
      options: [value],
      anchor: root,
      readValue: () => value,
      writeValue: writes,
      disabled: () => false,
      subscribe: () => () => undefined,
    },
    [{ ...item, id: 'a'.repeat(64) }],
    {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      classifyContent: async () => undefined,
      refreshMetadata: async () => undefined,
      replaceThumbnail: upload,
      releases: async () => [],
      acquire: async () => '21.safetensors',
    },
    feedback,
    null,
    null,
    {
      choices: () => [
        {
          id: 'image',
          sceneKey: '',
          sceneTitle: '',
          sceneOrder: 0,
          sourceKey: 'output',
          sourceLabel: 'Result',
          batch: 1,
        },
      ],
      activeChoice: () => null,
      image: readImage,
    },
  );
  try {
    root
      .querySelector('input')
      ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    const command = [...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')].find(
      (element) => element.textContent === 'Set thumbnail from canvas',
    );
    if (!command) throw new Error('Missing output image command.');
    command.click();
    expect(readImage).toHaveBeenCalledTimes(1);
    picker.dispose();
    expect(readImage.mock.calls[0]?.[1].aborted).toBe(true);
    finishImage(new File(['fixture'], 'output.png', { type: 'image/png' }));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(upload).not.toHaveBeenCalled();
    expect(feedback).not.toHaveBeenCalled();
    expect(writes).not.toHaveBeenCalled();
  } finally {
    finishImage(new File(['fixture'], 'output.png', { type: 'image/png' }));
    picker.dispose();
    root.remove();
  }
});
