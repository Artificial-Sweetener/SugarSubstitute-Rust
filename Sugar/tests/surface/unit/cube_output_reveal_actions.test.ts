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
/** Prove Cube thumbnail inspection complements download/context controls and excludes transient frames. */
import { jest } from '@jest/globals';
import { CubePreviewRailView } from '../../../frontend/comfyui/ui/surface/CubePreviewRailView.js';
import {
  ComfyCubePreviewActions,
  type ComfyPreviewMenuOption,
} from '../../../frontend/comfyui/ui/surface/ComfyCubePreviewActions.js';
import type { CubePreviewItem } from '../../../frontend/comfyui/ui/surface/CubePreviewModel.js';

const item: CubePreviewItem = {
  key: 'final',
  url: '/api/view?filename=final.png&type=output',
  label: '<img src=x>',
  revealTarget: {
    sessionId: 'workflow:proof',
    generation: 0,
    sourceId: 'source:cube',
    runOrder: 1,
    itemId: 'result:proof',
    batchIndex: 2,
  },
};

test('routes the exact captured identity and keeps Cube download and literal text independent', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const actions = { download: jest.fn(), reveal: jest.fn(), openContextMenu: jest.fn() };
  const rail = new CubePreviewRailView(root, actions);
  try {
    rail.render({ outputs: [{ id: 'image', label: 'image', items: [item] }] });
    const reveal = root.querySelector<HTMLButtonElement>(
      'button[aria-label="Show in output canvas"]',
    )!;
    const download = root.querySelector<HTMLButtonElement>('button[aria-label="Download image"]')!;
    reveal.click();
    download.click();
    expect(actions.reveal).toHaveBeenCalledWith(item);
    expect(actions.download).toHaveBeenCalledWith(item);
    expect(root.querySelector('figure')?.getAttribute('aria-label')).toBe(item.label);
    expect(root.querySelectorAll('img')).toHaveLength(1);
    rail.render({
      outputs: [
        {
          id: 'image',
          label: 'image',
          items: [{ key: 'live', url: 'blob:preview', label: 'image' }],
        },
      ],
    });
    expect(root.querySelector('button[aria-label="Show in output canvas"]')).toBeNull();
  } finally {
    rail.dispose();
    root.remove();
  }
});

test('adds exact inspection to the Cube menu without replacing native open/copy/save commands', () => {
  let menu: readonly ComfyPreviewMenuOption[] = [];
  const revealImage = jest.fn(async () => true);
  const actions = new ComfyCubePreviewActions({
    document,
    openWindow: jest.fn(),
    writeClipboard: async () => {},
    fetchBlob: async () => new Blob(),
    showContextMenu: (options) => {
      menu = options;
    },
    reportError: jest.fn(),
    revealImage,
  });
  actions.openContextMenu(item, new MouseEvent('contextmenu'));
  expect(menu.map((entry) => entry.content)).toEqual([
    'Show in output canvas',
    'Open Image',
    'Copy Image',
    'Save Image',
  ]);
  menu[0]!.callback();
  expect(revealImage).toHaveBeenCalledWith(item.revealTarget);
  actions.openContextMenu(
    { key: 'live', url: 'blob:preview', label: 'live' },
    new MouseEvent('contextmenu'),
  );
  expect(menu.map((entry) => entry.content)).toEqual(['Open Image', 'Copy Image', 'Save Image']);
});
