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
/** Prove title and field entry points use one menu without changing card geometry or values. */
import { expect, test } from '@jest/globals';
import { EditorContextMenuHost } from '../../../frontend/comfyui/ui/editor_actions/presentation/EditorContextMenuHost.js';
import { UserPresetLibrary } from '../../../frontend/comfyui/ui/editor_actions/application/UserPresetLibrary.js';
import type { NativeEditorCard } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeEditorCardAdapter.js';

/** Mount a projected card using the shared Cube-face and workflow-masonry boundary. */
function mountProjectedCard(root: HTMLElement): HTMLElement {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card sugarcubes-cube-face__native-card-mount';
  mount.append(root);
  document.body.append(mount);
  return mount;
}
test('header and dimension right-click share policy without adding a cog and shutdown releases ownership', async () => {
  const root = document.createElement('div');
  root.className = 'lg-node';
  const header = document.createElement('div');
  header.className = 'lg-node-header';
  root.append(header);
  const widthElement = document.createElement('div'),
    heightElement = document.createElement('div');
  root.append(widthElement, heightElement);
  const mount = mountProjectedCard(root);
  let width = 1024,
    height = 1024;
  const card: NativeEditorCard = {
    modelInputs: [],
    root,
    header,
    title: 'Latent',
    nodeType: 'Latent',
    advanced: null,
    fields: [
      {
        name: 'width',
        kind: 'number',
        element: widthElement,
        read: () => width,
        write: (value) => {
          if (typeof value !== 'number') return false;
          width = value;
          return true;
        },
        disabled: () => false,
      },
      {
        name: 'height',
        kind: 'number',
        element: heightElement,
        read: () => height,
        write: (value) => {
          if (typeof value !== 'number') return false;
          height = value;
          return true;
        },
        disabled: () => false,
      },
    ],
  };
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  let enabled = true;
  const host = new EditorContextMenuHost({
    document,
    library,
    readCard: () => card,
    families: () => [],
    reportError: () => undefined,
    skipped: () => undefined,
    enabled: () => enabled,
    modelContributions: () => [{ fieldName: 'model', entries: [{ label: 'Go to CivitAI page' }] }],
  });
  host.setup();
  await Promise.resolve();
  await Promise.resolve();
  const cog = header.querySelector<HTMLButtonElement>('[data-sc-node-actions]');
  expect(cog).toBeNull();
  widthElement.dispatchEvent(
    new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 150, clientY: 120 }),
  );
  expect(document.querySelector('[role=menu]')?.parentElement).toBe(document.body);
  expect(document.querySelector('[role=menu]')?.textContent).toContain('Set ratio by Width');
  expect(document.querySelector('[role=menu]')?.textContent).not.toContain('Set ratio by Height');
  expect([width, height]).toEqual([1024, 1024]);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  header.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  expect(document.querySelector('[role=menu]')?.textContent).toContain('Set ratio by Height');
  expect(root.querySelector('[role=menu]')).toBeNull();
  expect(document.querySelector('[role=menu]')?.textContent).toContain('Go to CivitAI page');
  enabled = false;
  host.refresh();
  await Promise.resolve();
  expect(document.querySelector('[role=menu]')).toBeNull();
  expect(header.querySelector('[data-sc-node-actions]')).toBeNull();
  enabled = true;
  host.refresh();
  await Promise.resolve();
  header.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  expect(document.querySelector('[role=menu]')).not.toBeNull();
  document.body.append(root);
  await Promise.resolve();
  await Promise.resolve();
  expect(document.querySelector('[role=menu]')).toBeNull();
  expect(host.preparedCards()).toEqual([]);
  host.dispose();
  expect(header.querySelector('[data-sc-node-actions]')).toBeNull();
  expect(document.querySelector('[role=menu]')).toBeNull();
  root.remove();
  mount.remove();
});

test('retains the host menu when every declared input is protected and no command is available', async () => {
  const root = document.createElement('div');
  root.className = 'lg-node';
  const header = document.createElement('div');
  root.append(header);
  const mount = mountProjectedCard(root);
  const card: NativeEditorCard = {
    modelInputs: [],
    root,
    header,
    title: 'Connected',
    nodeType: 'Connected',
    advanced: null,
    fields: [
      {
        name: 'value',
        kind: 'number',
        element: root,
        read: () => 12,
        disabled: () => true,
        write: () => false,
      },
    ],
  };
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  const host = new EditorContextMenuHost({
    document,
    library,
    readCard: () => card,
    families: () => [],
    reportError: () => undefined,
    skipped: () => undefined,
  });
  host.setup();
  await Promise.resolve();
  expect(header.querySelector('[data-sc-node-actions]')).toBeNull();
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  header.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
  expect(host.preparedCards()).toEqual([card]);
  host.dispose();
  root.remove();
  mount.remove();
});

test('ordinary Nodes 2.0 graph cards retain Comfy ownership even when an injectable reader recognizes them', async () => {
  const root = document.createElement('div'),
    header = document.createElement('div');
  root.className = 'lg-node';
  root.append(header);
  document.body.append(root);
  let reads = 0;
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  const host = new EditorContextMenuHost({
    document,
    library,
    families: () => [],
    reportError: () => undefined,
    skipped: () => undefined,
    readCard: () => {
      reads += 1;
      return {
        modelInputs: [],
        root,
        header,
        title: 'Graph node',
        nodeType: 'Graph node',
        fields: [],
        advanced: { shown: () => false, setShown: () => undefined },
      };
    },
  });
  host.setup();
  await Promise.resolve();
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  header.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
  expect(reads).toBe(0);
  expect(host.preparedCards()).toEqual([]);
  expect(document.querySelector('[role=menu]')).toBeNull();
  host.dispose();
  root.remove();
});
