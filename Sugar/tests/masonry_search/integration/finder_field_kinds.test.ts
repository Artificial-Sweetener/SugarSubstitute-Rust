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
/** Prove every kind of field shows its matches through its own renderer, with one current match. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { MasonrySearchPresenter } from '../../../frontend/comfyui/ui/search/presentation/MasonrySearchPresenter.js';
import type {
  SearchFieldSurface,
  SearchSurface,
} from '../../../frontend/comfyui/ui/search/presentation/SearchSurface.js';
import type { PromptSearchSurface } from '../../../frontend/comfyui/ui/prompt_editor/PromptSearchSurface.js';
import type { FinderInputControl } from '../../../frontend/comfyui/ui/surface/appearance/FinderControls.js';
import {
  anySearchHighlight,
  installSearchHighlights,
  boxRuns,
  resetSearchHighlights,
  shownCurrent,
  shownMatches,
} from '../support/SearchHighlightFixture.js';

const presenters: MasonrySearchPresenter[] = [];
installSearchHighlights();

afterEach(() => {
  for (const presenter of presenters.splice(0)) presenter.dispose();
  document.body.replaceChildren();
  resetSearchHighlights();
  jest.restoreAllMocks();
});

/** A prompt that records what the finder asks it to paint. */
function prompt(element: HTMLElement) {
  const calls = {
    painted: [] as { query: string; ranges: [number, number][]; active: number | null }[],
    revealed: [] as number[],
    focused: [] as number[],
    cleared: 0,
  };
  const surface: PromptSearchSurface = {
    element,
    paint: (query, ranges, active) =>
      calls.painted.push({
        query,
        ranges: ranges.map((range) => [range.start, range.length]),
        active,
      }),
    reveal: (index) => calls.revealed.push(index),
    focus: (index) => calls.focused.push(index),
    clear: () => {
      calls.cleared++;
    },
  };
  return { surface, calls };
}

/** One card: a titled node with a prompt, a plain text box and a value shown as text. */
function fixture() {
  const container = document.createElement('div');
  const root = document.createElement('div');
  const card = document.createElement('article');
  const title = document.createElement('h3');
  title.textContent = 'Load LoRA sunset';
  card.append(title);
  container.append(root);
  root.append(card);
  document.body.append(container);
  const row = (name: string, child: HTMLElement): { row: HTMLElement; label: HTMLElement } => {
    const element = document.createElement('div');
    const label = document.createElement('span');
    label.textContent = name;
    element.append(label, child);
    card.append(element);
    return { row: element, label };
  };
  const editor = document.createElement('div');
  const painter = prompt(editor);
  const box = document.createElement('input');
  box.value = 'sunset_render';
  const shown = document.createElement('b');
  shown.textContent = 'Sunset Style';
  const fields = new Map<string, SearchFieldSurface>([
    [
      'text',
      {
        id: 'text',
        node: 'n',
        card,
        ...row('text', editor),
        control: null,
        painter: painter.surface,
      },
    ],
    [
      'prefix',
      { id: 'prefix', node: 'n', card, ...row('prefix', box), control: box, painter: null },
    ],
    [
      'style',
      { id: 'style', node: 'n', card, ...row('lora_name', shown), control: null, painter: null },
    ],
  ]);
  const values: Record<string, string> = {
    text: 'a sunset over the harbour at Sunset',
    prefix: 'sunset_render',
    style: 'style\\sunset_style.safetensors',
  };
  const surface: SearchSurface = {
    fields,
    read: () => [
      {
        id: 'n',
        name: 'Load LoRA sunset',
        title: 'Load LoRA sunset',
        type: 'LoraLoader',
        fields: [...fields.values()].map((field) => ({
          id: field.id,
          key: field.id,
          label: field.label?.textContent ?? field.id,
          text: values[field.id] ?? null,
        })),
      },
    ],
  };
  const presenter = new MasonrySearchPresenter({ root, container, surface, setCards: jest.fn() });
  presenters.push(presenter);
  presenter.open();
  const input = container.querySelector<FinderInputControl>('.sc-masonry-finder__input');
  if (!input) throw new Error('Finder input missing');
  const search = (text: string): void => {
    input.value = text;
    input.control.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    void presenter.state;
  };
  return { presenter, painter: painter.calls, box, search, root };
}

test('a prompt ranges its own matches and the current match moves between renderers', () => {
  const f = fixture();
  f.search('sunset');
  // Two in the prompt, one in the text box; the first is current.
  expect(f.presenter.state.active).toMatchObject({ field: 'text', start: 2 });
  expect(f.painter.painted.at(-1)).toMatchObject({
    ranges: [
      [2, 6],
      [29, 6],
    ],
    active: 0,
  });
  expect(f.painter.revealed.at(-1)).toBe(0);
  // Outside the prompt, the text box's match and the shown value are matches, none current.
  expect(boxRuns(f.root)).toEqual(['sunset:match']);
  expect(shownMatches()).toEqual(['Sunset@B']);
  expect(shownCurrent()).toEqual([]);
  f.presenter.cycle(1);
  expect(f.painter.painted.at(-1)?.active).toBe(1);
  expect(f.painter.revealed.at(-1)).toBe(1);
  // The current match moves to the text box: the prompt keeps its matches, none of them current.
  f.presenter.cycle(1);
  expect(f.presenter.state.active?.field).toBe('prefix');
  expect(f.painter.painted.at(-1)?.active).toBeNull();
  expect(boxRuns(f.root)).toEqual(['sunset:current']);
  expect(shownMatches()).toEqual(['Sunset@B']);
  // It is the text box's real selection as well, which shows once the box has focus.
  expect([f.box.selectionStart, f.box.selectionEnd]).toEqual([0, 6]);
});

test('a text box marks its matches on a transparent copy of what it displays', () => {
  const f = fixture();
  f.search('sunset');
  const copy = f.root.querySelector('.sc-search-highlight__text');
  expect(copy?.textContent).toBe('sunset_render​');
  expect(f.root.querySelectorAll('mark')).toHaveLength(0);
  // The copy is searched as displayed: a box showing its value differently is marked on what it shows.
  f.box.value = 'A Sunset_render';
  f.presenter.cycle(1);
  expect(boxRuns(f.root)).toEqual(['Sunset:match']);
  expect(copy?.textContent).toBe('A Sunset_render​');
});

test('a new query starts a new search in the prompt and closing clears everything', () => {
  const f = fixture();
  f.search('sunset');
  const first = f.painter.painted.at(-1)?.query;
  f.search('harbour');
  expect(f.painter.painted.at(-1)?.query).not.toBe(first);
  expect(f.painter.painted.at(-1)?.ranges).toEqual([[18, 7]]);
  expect(f.root.querySelectorAll('.sc-search-highlight')).toHaveLength(0);
  expect(anySearchHighlight()).toBe(false);
  f.search('sunset');
  expect(anySearchHighlight()).toBe(true);
  f.presenter.close(false);
  expect(f.painter.cleared).toBeGreaterThan(0);
  expect(f.painter.focused).toEqual([]);
  expect(anySearchHighlight()).toBe(false);
  expect(f.root.querySelectorAll('.sc-search-highlight')).toHaveLength(0);
});

test('Escape lands in the prompt on the current match', () => {
  const f = fixture();
  f.search('sunset');
  f.presenter.cycle(1);
  f.presenter.close(true);
  expect(f.painter.focused).toEqual([1]);
});

test('Escape on a text box match selects it there', () => {
  const f = fixture();
  f.search('render');
  expect(f.presenter.state.active?.field).toBe('prefix');
  f.presenter.close(true);
  expect(document.activeElement).toBe(f.box);
  expect([f.box.selectionStart, f.box.selectionEnd]).toEqual([7, 13]);
});

test('a value shown as text is ranged where it reads the searched text', () => {
  const f = fixture();
  f.search('style');
  // The model field shows its title, so only the characters on screen are ranged.
  expect(shownMatches()).toEqual(['Style@B']);
});

test('Node and Field queries narrow what is shown and highlight nothing', () => {
  const f = fixture();
  f.search('@node lora');
  expect(anySearchHighlight()).toBe(false);
  expect(f.painter.painted).toEqual([]);
  f.search('@field name lora');
  expect(anySearchHighlight()).toBe(false);
  // Text searched within a named node is still text: its matches are shown.
  f.search('@node lora "harbour"');
  expect(f.painter.painted.at(-1)?.ranges).toEqual([[18, 7]]);
  expect(shownMatches()).toEqual([]);
});
