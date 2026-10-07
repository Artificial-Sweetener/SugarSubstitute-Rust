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
/** Prove host presentation changes cannot invalidate workflow relation discovery. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { RelationHostLifecycle } from '../../../frontend/comfyui/ui/relations/RelationHostLifecycle.js';
import { NativeRelationInventory } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationInventory.js';
import { CubeRelationPresenter } from '../../../frontend/comfyui/ui/relations/presentation/CubeRelationPresenter.js';
import { refreshNativeNodeCards } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRefresh.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import type { ObservableNativeSubgraph } from '../../../frontend/comfyui/ui/surface/NativeSubgraphChangeObserver.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Drain native forwarding and mutation batches without replacing their owners. */
async function settle(): Promise<void> {
  for (let index = 0; index < 6; index++) await Promise.resolve();
}

test('dock placeholders and native Run content do not refresh cards or rebuild field inventory', async () => {
  const inventory = jest.spyOn(NativeRelationInventory.prototype, 'read');
  const cards = jest.spyOn(CubeRelationPresenter.prototype, 'reconcile');
  const host = new RelationHostLifecycle({
    app: { graph: { _nodes: [], extra: {} } },
    document,
    logger: console,
    reportError: jest.fn(),
    getCardState: () => null,
  });
  host.setup();
  await settle();
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  document.body.append(bar);
  await settle();
  expect(cards).toHaveBeenCalled();
  inventory.mockClear();
  cards.mockClear();
  const target = document.createElement('div');
  target.className = 'sc-overrides-dock-target';
  bar.append(target);
  await settle();
  target.textContent = 'Dock to Run';
  target.toggleAttribute('data-sc-override-drop-target', true);
  await settle();
  target.remove();
  bar.append(document.createElement('button'));
  await settle();
  expect(inventory).not.toHaveBeenCalled();
  expect(cards).not.toHaveBeenCalled();
  host.dispose();
});

test('ignores popup mounts, search input, and unrelated DOM replacement', async () => {
  const inventory = jest.spyOn(NativeRelationInventory.prototype, 'read');
  const errors = jest.fn();
  const host = new RelationHostLifecycle({
    app: { graph: { _nodes: [], extra: {} } },
    document,
    logger: console,
    reportError: errors,
    getCardState: () => null,
  });
  host.setup();
  await settle();
  inventory.mockClear();
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  const search = document.createElement('input');
  popup.append(search);
  document.body.append(popup);
  await settle();
  search.value = 'euler';
  search.dispatchEvent(new Event('input', { bubbles: true }));
  popup.append(document.createElement('div'));
  await settle();
  popup.remove();
  await settle();
  expect(inventory).not.toHaveBeenCalled();
  expect(errors).not.toHaveBeenCalled();
  host.dispose();
});

test('binds newly mounted native surfaces without rediscovering workflow fields', async () => {
  const inventory = jest.spyOn(NativeRelationInventory.prototype, 'read');
  const host = new RelationHostLifecycle({
    app: { graph: { _nodes: [], extra: {} } },
    document,
    logger: console,
    reportError: jest.fn(),
    getCardState: () => null,
  });
  host.setup();
  await settle();
  inventory.mockClear();
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.nodeId = '1';
  document.body.append(root);
  await settle();
  expect(inventory).not.toHaveBeenCalled();
  host.afterConfigureGraph();
  await settle();
  expect(inventory).toHaveBeenCalledTimes(1);
  inventory.mockClear();
  refreshNativeNodeCards(document, { kind: 'reset', reason: 'native-choices' });
  await settle();
  expect(inventory).toHaveBeenCalledTimes(1);
  host.dispose();
});

test('forwards Nodes 1 native DOM-widget value edits once through retained bindings', async () => {
  const inventory = jest.spyOn(NativeRelationInventory.prototype, 'read');
  const host = new RelationHostLifecycle({
    app: { graph: { _nodes: [], extra: {} } },
    document,
    logger: console,
    reportError: jest.fn(),
    getCardState: () => null,
  });
  host.setup();
  await settle();
  inventory.mockClear();
  const reconcile = jest.spyOn(host.relations, 'reconcile');
  const widget = document.createElement('div');
  widget.className = 'dom-widget';
  const input = document.createElement('textarea');
  widget.append(input);
  document.body.append(widget);
  await settle();
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
  expect(reconcile).toHaveBeenCalledTimes(1);
  expect(reconcile).toHaveBeenCalledWith(false);
  expect(inventory).not.toHaveBeenCalled();
  host.dispose();
});

test('known relation transactions bypass rediscovery while native topology and undo still notify', async () => {
  const graph: ComfyGraph & ObservableNativeSubgraph = {
    _nodes: [],
    extra: {},
    add: () => undefined,
    remove: () => undefined,
    afterChange: () => graph.onAfterChange?.(graph),
  };
  const host = new RelationHostLifecycle({
    app: { graph },
    document,
    logger: console,
    reportError: jest.fn(),
    getCardState: () => null,
  });
  host.setup();
  await settle();
  const inventory = jest.spyOn(NativeRelationInventory.prototype, 'read');
  host.relations.linkNode('removed', null);
  await settle();
  expect(inventory).not.toHaveBeenCalled();
  graph.onAfterChange?.(graph);
  await settle();
  expect(inventory).toHaveBeenCalledTimes(1);
  graph.add?.({ id: 1 });
  await settle();
  expect(inventory).toHaveBeenCalledTimes(2);
  host.dispose();
});
