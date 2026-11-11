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
/** Prove finder discovery stays scoped to policy-visible mounted cards and host lifecycles. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { MasonrySearchCorpus } from '../../../frontend/comfyui/ui/search/infrastructure/MasonrySearchCorpus.js';
import type { MasonrySearchSurface } from '../../../frontend/comfyui/ui/search/infrastructure/MasonrySearchHost.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import type {
  ComfyGraph,
  ComfyNode,
  ComfyWidget,
} from '../../../frontend/comfyui/ui/types/graph.js';
import type { FinderInputControl } from '../../../frontend/comfyui/ui/surface/appearance/FinderControls.js';

import { MasonrySearchHost } from '../../../frontend/comfyui/ui/search/infrastructure/MasonrySearchHost.js';
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Model an exposed native card plus consumed, hidden and structured values at the host boundary. */
function fixture() {
  const graph: ComfyGraph = { id: 'workflow', _nodes: [] };
  const node: ComfyNode = {
    id: 1,
    type: 'Sampler',
    widgets: [
      { name: 'seed', type: 'number', value: 512 },
      { name: 'connected', type: 'number', value: 512 },
      { name: 'private', type: 'text', value: 'hidden' },
      { name: 'structured', type: 'text', value: { html: '<b>secret</b>' } },
    ],
    inputs: [{ widget: { name: 'connected' }, link: 2 }],
    isWidgetVisible: (widget: ComfyWidget) => widget.name !== 'private',
  };
  const container = document.createElement('div');
  const element = document.createElement('section');
  const cell = document.createElement('article');
  cell.dataset.masonryCard = 'card';
  const card = document.createElement('div');
  card.className = 'lg-node';
  for (const widget of node.widgets ?? []) {
    const row = document.createElement('div');
    row.dataset.scStyleRow = '';
    row.dataset.testid = 'node-widget';
    const input = document.createElement('input');
    input.setAttribute('aria-label', widget.name);
    input.value = String(widget.value);
    row.append(input);
    card.append(row);
  }
  cell.append(card);
  element.append(cell);
  container.append(element);
  document.body.append(container);
  const sections: WorkflowMasonrySection[] = [
    {
      id: 'loose',
      label: 'Workflow nodes',
      cube: null,
      target: graph,
      cards: [
        {
          id: 'card',
          node,
          label: 'Sampler',
          visible: true,
          enabled: true,
          showActivationControl: false,
          columnSpan: 1,
        },
      ],
    },
  ];
  const cells = new Map([['card', cell]]);
  const view: MasonrySearchSurface = {
    element,
    sections,
    cards: () => cells,
    setSearchCards: jest.fn(),
  };
  return { graph, node, container, view, sections, cells, card };
}

test('includes only exposed editable widget values and never invents text for structured payloads', () => {
  const f = fixture();
  const corpus = new MasonrySearchCorpus(f.sections, f.cells);
  expect([...corpus.fields.keys()]).toEqual(['["card","seed"]', '["card","structured"]']);
  expect(corpus.read()[0]?.fields.map((field) => field.text)).toEqual(['512', null]);
  const lookup = jest.spyOn(f.card, 'querySelectorAll');
  corpus.read();
  corpus.read();
  expect(lookup).not.toHaveBeenCalled();
});

test('uses forwarded face values and excludes unprojected or policy-hidden cards', () => {
  const f = fixture();
  const face = f.sections[0]?.cards[0];
  if (!face) throw new Error('Projected card missing');
  const source: ComfyNode = { id: 2, widgets: [{ name: 'value', type: 'number', value: 123 }] };
  face.widgetSources = [{ node: source, name: 'value', targetName: 'connected' }];
  const corpus = new MasonrySearchCorpus(f.sections, f.cells);
  expect(corpus.read()[0]?.fields.find((field) => field.key === 'connected')?.text).toBe('123');
  face.visible = false;
  const hidden = new MasonrySearchCorpus(f.sections, f.cells);
  expect(hidden.read()).toEqual([]);
  expect(hidden.fields.size).toBe(0);
});

test('Ctrl+F ownership, popup isolation and workflow-scoped restoration release every mounted reference', async () => {
  const f = fixture();
  const reportError = jest.fn();
  const host = new MasonrySearchHost({ document, app: null, getGraph: () => f.graph, reportError });
  host.setup();
  host.ready(f.view);
  const shortcut = new KeyboardEvent('keydown', {
    key: 'f',
    ctrlKey: true,
    bubbles: true,
    cancelable: true,
  });
  window.dispatchEvent(shortcut);
  expect(shortcut.defaultPrevented).toBe(true);
  const input = f.container.querySelector<FinderInputControl>('.sc-masonry-finder__input');
  if (!input) throw new Error('Finder missing');
  input.value = '@node sampler "512"';
  input.control.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  const lookup = jest.spyOn(f.card, 'querySelectorAll');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
  expect(lookup).not.toHaveBeenCalled();
  host.release();
  expect(f.container.querySelector('.sc-masonry-finder')).toBeNull();
  expect(f.container.querySelector('.sc-search-highlight')).toBeNull();
  host.ready(f.view);
  const restored = f.container.querySelector<FinderInputControl>('.sc-masonry-finder__input');
  expect(restored?.value).toBe('sampler "512"');
  expect(restored?.parentElement?.hidden).toBe(false);
  expect(reportError).not.toHaveBeenCalled();
  host.dispose();
  const graphShortcut = new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, cancelable: true });
  window.dispatchEvent(graphShortcut);
  expect(graphShortcut.defaultPrevented).toBe(false);
});

test('visible modals own Ctrl+F while retained hidden dialogs do not block the finder', () => {
  const f = fixture();
  const host = new MasonrySearchHost({
    document,
    app: null,
    getGraph: () => f.graph,
    reportError: jest.fn(),
  });
  const modal = document.createElement('div');
  modal.setAttribute('role', 'dialog');
  modal.setAttribute('aria-modal', 'true');
  let visible = true;
  modal.checkVisibility = jest.fn(() => visible);
  document.body.append(modal);
  host.setup();
  host.ready(f.view);
  try {
    const blocked = new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, cancelable: true });
    window.dispatchEvent(blocked);
    expect(blocked.defaultPrevented).toBe(false);
    expect(f.container.querySelector('.sc-masonry-finder')).toBeNull();
    visible = false;
    const allowed = new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, cancelable: true });
    window.dispatchEvent(allowed);
    expect(allowed.defaultPrevented).toBe(true);
    expect(f.container.querySelector<HTMLElement>('.sc-masonry-finder')?.hidden).toBe(false);
  } finally {
    host.dispose();
  }
});
