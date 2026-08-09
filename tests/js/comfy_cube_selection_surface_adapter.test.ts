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
/** Verify Vue toolbox and More Options adaptation through stable host anchors. */

import { jest } from '@jest/globals';
import { ComfyCubeSelectionSurfaceAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeSelectionSurfaceAdapter.js';
import { ComfyCubeSelectionToolboxAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeSelectionToolboxAdapter.js';
import { ComfyCubeSelectionMenuVisibilityAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeSelectionMenuVisibilityAdapter.js';
import { ComfyCubeCardVisibilityToolboxPresenter } from '../../frontend/comfyui/ui/affordance/ComfyCubeCardVisibilityToolboxPresenter.js';
import { ComfyCubeSaveButtonPresenter } from '../../frontend/comfyui/ui/affordance/ComfyCubeSaveButtonPresenter.js';
import { PrimeVueTooltipPresentationAdapter } from '../../frontend/comfyui/ui/affordance/PrimeVueTooltipPresentationAdapter.js';
import { ComfyToolboxTooltipPresenter } from '../../frontend/comfyui/ui/affordance/ComfyToolboxTooltipPresenter.js';
import { ComfyToolboxCheckMenuPresenter } from '../../frontend/comfyui/ui/affordance/ComfyToolboxCheckMenuPresenter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

test('hides structural actions and relabels native Cube actions without parsing labels', () => {
  document.body.replaceChildren();
  const toolbox = element('div', { 'data-testid': 'selection-toolbox' });
  const unpack = buttonWithIcon('icon-[lucide--expand]');
  unpack.setAttribute('data-testid', 'convert-to-subgraph-button');
  const publish = buttonWithIcon('icon-[lucide--book-open]');
  publish.setAttribute('aria-label', 'Publish Subgraph');
  Reflect.set(publish, '$_ptooltipValue', 'Publish Subgraph');
  publish.firstElementChild?.setAttribute('style', 'display: inline-flex !important');
  const configure = buttonWithIcon('icon-[lucide--settings-2]');
  toolbox.append(unpack, publish, configure);
  const menu = element('div', { class: 'p-contextmenu' });
  const libraryItem = element('li');
  libraryItem.append(element('i', { class: 'icon-[lucide--folder-plus]' }));
  const infoItem = element('li');
  infoItem.append(element('i', { class: 'pi pi-info-circle' }));
  menu.append(libraryItem, infoItem);
  document.body.append(toolbox, menu);
  const cube = cubeNode();
  const canvas = { selectedItems: new Set<unknown>([cube]) };
  const setRevealed = jest.fn();
  const adapter = new ComfyCubeSelectionSurfaceAdapter({
    document,
    canvas,
    contexts: new CubeEditorContextResolver(new CubeNodeCatalog()),
    toolbox: new ComfyCubeSelectionToolboxAdapter({
      document,
      saveButton: new ComfyCubeSaveButtonPresenter({
        document,
        tooltips: new PrimeVueTooltipPresentationAdapter({ document, logger: console }),
      }),
      cardVisibility: new ComfyCubeCardVisibilityToolboxPresenter({
        document,
        owner: {
          list: () => [{ id: 'models', label: 'Models', revealed: false }],
          setRevealed,
        },
        tooltip: new ComfyToolboxTooltipPresenter(document),
        menu: new ComfyToolboxCheckMenuPresenter({
          document,
          featureMenuAttribute: 'data-sugarcubes-card-visibility-menu',
          featureItemAttribute: 'data-sugarcubes-card-visibility-row',
        }),
      }),
      versions: {
        present: (anchor) => anchor,
        clear: jest.fn(),
        dispose: jest.fn(),
      },
    }),
    menus: new ComfyCubeSelectionMenuVisibilityAdapter(document),
  });

  adapter.refresh();
  expect(unpack.hidden).toBe(true);
  expect(libraryItem.hidden).toBe(true);
  expect(libraryItem.style.getPropertyPriority('display')).toBe('important');
  expect(infoItem.hidden).toBe(true);
  expect(publish.getAttribute('aria-label')).toBe('Save Cube');
  expect(Reflect.get(publish, '$_ptooltipValue')).toBe('Save Cube');
  expect(publish.hasAttribute('title')).toBe(false);
  expect(publish.querySelector<HTMLElement>('i[class="icon-[lucide--book-open]"]')?.hidden).toBe(
    true,
  );
  expect(
    publish
      .querySelector<HTMLElement>('i[class="icon-[lucide--book-open]"]')
      ?.style.getPropertyValue('display'),
  ).toBe('none');
  const saveIcon = publish.querySelector<HTMLElement>('[data-sugarcubes-save-cube-icon]');
  expect(saveIcon?.getAttribute('aria-hidden')).toBe('true');
  expect(saveIcon?.querySelector('i[class="icon-[lucide--box]"]')).not.toBeNull();
  const saveBadge = saveIcon?.querySelector<HTMLElement>('[data-sugarcubes-save-cube-badge]');
  expect(document.getElementById('sugarcubes-save-cube-button-styles')?.textContent).toContain(
    'background: var(--comfy-menu-bg, rgb(24 24 27))',
  );
  expect(saveBadge?.querySelector('i[class="icon-[lucide--save]"]')).not.toBeNull();
  const visibility = toolbox.querySelector<HTMLButtonElement>(
    '[data-sugarcubes-card-visibility-button]',
  );
  expect(visibility?.previousElementSibling).toBe(publish);
  expect(visibility?.getAttribute('aria-label')).toBe('Manage optional nodes');
  expect(visibility?.querySelector('.pi.pi-eye')).not.toBeNull();
  visibility?.dispatchEvent(new Event('pointerenter'));
  const visibilityTooltip = document.querySelector<HTMLElement>(
    '[data-sugarcubes-toolbox-tooltip]',
  );
  expect(visibilityTooltip?.getAttribute('role')).toBe('tooltip');
  expect(visibilityTooltip?.classList.contains('p-tooltip')).toBe(true);
  expect(visibilityTooltip?.querySelector('.p-tooltip-text')?.textContent).toBe(
    'Manage optional nodes',
  );
  visibility?.click();
  expect(document.querySelector('[data-sugarcubes-toolbox-tooltip]')).toBeNull();
  const visibilityMenu = document.querySelector<HTMLElement>(
    '[data-sugarcubes-card-visibility-menu]',
  );
  expect(visibilityMenu?.parentElement).toBe(document.body);
  expect(visibilityMenu?.getAttribute('role')).toBe('menu');
  const models = visibilityMenu?.querySelector<HTMLButtonElement>(
    '[data-sugarcubes-card-visibility-row="models"]',
  );
  expect(models?.textContent).toBe('Models');
  expect(models?.getAttribute('aria-checked')).toBe('false');
  models?.click();
  expect(setRevealed).toHaveBeenCalledWith(cube, 'models', true);
  adapter.refresh();
  expect(publish.querySelectorAll('[data-sugarcubes-save-cube-icon]')).toHaveLength(1);
  expect(toolbox.querySelectorAll('[data-sugarcubes-card-visibility-button]')).toHaveLength(1);
  expect(configure.hidden).toBe(true);
  canvas.selectedItems = new Set();
  adapter.refresh();
  expect(unpack.hidden).toBe(false);
  expect(libraryItem.hidden).toBe(false);
  expect(libraryItem.style.display).toBe('');
  expect(publish.getAttribute('aria-label')).toBe('Publish Subgraph');
  expect(Reflect.get(publish, '$_ptooltipValue')).toBe('Publish Subgraph');
  expect(publish.hasAttribute('title')).toBe(false);
  expect(publish.querySelector<HTMLElement>('i[class="icon-[lucide--book-open]"]')?.hidden).toBe(
    false,
  );
  expect(
    publish
      .querySelector<HTMLElement>('i[class="icon-[lucide--book-open]"]')
      ?.style.getPropertyValue('display'),
  ).toBe('inline-flex');
  expect(
    publish
      .querySelector<HTMLElement>('i[class="icon-[lucide--book-open]"]')
      ?.style.getPropertyPriority('display'),
  ).toBe('important');
  expect(publish.querySelector('[data-sugarcubes-save-cube-icon]')).toBeNull();
  expect(document.querySelector('[data-sugarcubes-card-visibility-button]')).toBeNull();
  expect(document.querySelector('[data-sugarcubes-card-visibility-menu]')).toBeNull();
  expect(configure.hidden).toBe(false);
});

/** Create one element with literal attributes. */
function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Record<string, string> = {},
): HTMLElementTagNameMap[K] {
  const value = document.createElement(tag);
  for (const [name, content] of Object.entries(attributes)) value.setAttribute(name, content);
  return value;
}

/** Create one button around a semantic Comfy icon class. */
function buttonWithIcon(iconClass: string): HTMLButtonElement {
  const button = document.createElement('button');
  button.append(element('i', { class: iconClass }));
  return button;
}

/** Build one complete marked Cube selection operand. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube',
      _nodes: [{ id: 'models', type: 'Models', mode: 4 }],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
