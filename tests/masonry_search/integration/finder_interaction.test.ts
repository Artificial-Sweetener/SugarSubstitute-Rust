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
/** Prove the mounted Fluent finder uses real native controls and preserves portable intent. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { MasonrySearchPresenter } from '../../../frontend/comfyui/ui/search/presentation/MasonrySearchPresenter.js';
import type {
  SearchFieldSurface,
  SearchSurface,
} from '../../../frontend/comfyui/ui/search/presentation/SearchSurface.js';
import type { FluentFinderInput } from '../../../frontend/comfyui/ui/surface/appearance/FluentFinderControls.js';
import { SearchVisibility } from '../../../frontend/comfyui/ui/search/presentation/SearchVisibility.js';
import { matchSearch } from '../../../frontend/comfyui/ui/search/domain/SearchMatching.js';
import { buildSearchQuery } from '../../../frontend/comfyui/ui/search/domain/SearchQuery.js';

const presenters: MasonrySearchPresenter[] = [];
afterEach(() => {
  for (const presenter of presenters.splice(0)) presenter.dispose();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Mount two editable controls while keeping source reads measurable independently of navigation. */
function fixture() {
  const container = document.createElement('div');
  const root = document.createElement('div');
  const card = document.createElement('article');
  container.append(root);
  root.append(card);
  document.body.append(container);
  const fields = new Map<string, SearchFieldSurface>();
  for (const id of ['width', 'height']) {
    const row = document.createElement('div');
    const control = document.createElement('input');
    control.value = '512';
    row.append(control);
    card.append(row);
    fields.set(id, { id, node: 'latent', card, row, control });
  }
  const read = jest.fn(() => [
    {
      id: 'latent',
      name: 'Latent',
      title: 'Latent',
      type: 'EmptyLatentImage',
      fields: [...fields.values()].map((field) => ({
        id: field.id,
        key: field.id,
        label: field.id,
        text: field.control?.value ?? null,
      })),
    },
  ]);
  const surface: SearchSurface = { fields, read };
  const setCards = jest.fn<(nodes: ReadonlySet<string> | null, remeasure: boolean) => void>();
  const presenter = new MasonrySearchPresenter({ root, container, surface, setCards });
  presenters.push(presenter);
  presenter.open();
  const input = container.querySelector<FluentFinderInput>('.sc-masonry-finder__input');
  if (!input) throw new Error('Finder input missing');
  /** Send actual composed native input events and flush the pending animation-frame query. */
  const search = (text: string): void => {
    input.value = text;
    input.control.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    void presenter.state;
  };
  /** Activate visible controls through the same click event used by Microsoft's component. */
  const click = (label: string): void => {
    const button = container.querySelector<HTMLElement>(`[aria-label="${label}"]`);
    if (!button) throw new Error(`Missing button: ${label}`);
    button.click();
  };
  return { container, root, fields, presenter, input, search, click, read, surface, setCards };
}

test('buttons cycle and wrap cached matches, clear and close without replacing native controls', () => {
  const f = fixture();
  f.search('512');
  expect(f.presenter.state.active?.field).toBe('width');
  const reads = f.read.mock.calls.length;
  f.click('Next match (Enter)');
  expect(f.presenter.state.active?.field).toBe('height');
  f.click('Next match (Enter)');
  expect(f.presenter.state.active?.field).toBe('width');
  f.click('Previous match (Shift+Enter)');
  expect(f.presenter.state.active?.field).toBe('height');
  expect(f.read).toHaveBeenCalledTimes(reads);
  expect(f.fields.get('height')?.control?.selectionEnd).toBe(3);
  f.click('Clear search');
  expect(f.presenter.state.text).toBe('');
  expect(f.root.querySelectorAll('mark')).toHaveLength(0);
  expect(f.input.shadowRoot?.activeElement).toBe(f.input.control);
  f.click('Close search (Esc)');
  expect(f.container.querySelector<HTMLElement>('.sc-masonry-finder')?.hidden).toBe(true);
  expect(f.fields.get('width')?.control?.value).toBe('512');
});

test('restores mouse reachability inside Comfy’s noninteractive overlay container', () => {
  const f = fixture();
  f.container.style.pointerEvents = 'none';
  const finder = f.container.querySelector<HTMLElement>('.sc-masonry-finder');
  if (!finder) throw new Error('Finder missing');
  expect(getComputedStyle(finder).pointerEvents).toBe('auto');
});

test('context trigger toggles its menu without changing query, matches or native control identity', () => {
  const f = fixture();
  f.search('512');
  const state = f.presenter.state;
  const reads = f.read.mock.calls.length;
  const trigger = f.container.querySelector<HTMLElement>('[aria-label="Search context"]');
  if (!trigger) throw new Error('Context trigger missing');
  f.click('Search context');
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  expect(document.querySelector('[role="menu"][aria-label="Search context"]')).not.toBeNull();
  trigger.dispatchEvent(new Event('pointerdown', { bubbles: true, composed: true }));
  f.click('Search context');
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  expect(document.querySelector('[role="menu"][aria-label="Search context"]')).toBeNull();
  expect(f.presenter.state).toEqual(state);
  expect(f.read).toHaveBeenCalledTimes(reads);
  expect(f.container.querySelector('.sc-masonry-finder__input')).toBe(f.input);
  expect(document.activeElement).toBe(trigger);
  f.click('Search context');
  expect(trigger.getAttribute('aria-expanded')).toBe('true');
  f.click('Close search (Esc)');
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  expect(document.querySelector('[role="menu"][aria-label="Search context"]')).toBeNull();
});

test('context selector provides a Fluent stroke divider without intercepting search input', () => {
  const f = fixture();
  const trigger = f.container.querySelector<HTMLElement>('.sc-masonry-finder__mode');
  if (!trigger) throw new Error('Context trigger missing');
  expect(getComputedStyle(trigger).borderRightWidth).toBe('1px');
  expect(getComputedStyle(trigger).borderRightStyle).toBe('solid');
  expect(document.getElementById('sc-masonry-finder-styles')?.textContent).toContain(
    'border-right-color:var(--colorNeutralStroke2)',
  );
  f.search('512');
  expect(f.presenter.state.active?.field).toBe('width');
  f.click('Next match (Enter)');
  expect(f.presenter.state.active?.field).toBe('height');
});

test('finder icon actions use official subtle rounded buttons over textured acrylic', () => {
  const f = fixture();
  const buttons = f.container.querySelectorAll('fluent-button:not(.sc-masonry-finder__mode)');
  expect(buttons).toHaveLength(5);
  for (const button of buttons) {
    expect(button.getAttribute('role')).toBe('button');
    expect(button.getAttribute('appearance')).toBe('subtle');
    expect(button.getAttribute('shape')).toBe('rounded');
    expect(button.hasAttribute('icon-only')).toBe(true);
    expect(getComputedStyle(button).width).toBe('28px');
    expect(getComputedStyle(button).height).toBe('28px');
  }
  const styles = document.getElementById('sc-masonry-finder-styles')?.textContent;
  expect(styles).toContain('backdrop-filter:blur(30px) saturate(125%)');
  expect(styles).toContain('acrylic-grain.svg');
  expect(styles).toContain('var(--colorNeutralBackground2) 78%');
  expect(styles).toContain('gap:0; background:transparent');
  expect(styles).toContain('prefers-reduced-transparency:reduce');
});

test('context choices return focus to the real Fluent input and Field masks surrender space', () => {
  const f = fixture();
  f.click('Search context');
  const field = document.querySelector<HTMLElement>('[role="menuitemradio"][aria-checked="false"]');
  if (!field) throw new Error('Field choice missing');
  expect(field.textContent).toBe('Field');
  field.click();
  void f.presenter.state;
  expect(f.presenter.state.mode).toBe('field');
  expect(f.input.shadowRoot?.activeElement).toBe(f.input.control);
  f.search('width');
  expect(f.fields.get('height')?.row.hasAttribute('data-sc-search-field-hidden')).toBe(true);
  expect(f.fields.get('width')?.row.hasAttribute('data-sc-search-field-hidden')).toBe(false);
  expect(f.setCards).toHaveBeenLastCalledWith(new Set(['latent']), true);
  f.click('Close search (Esc)');
  expect(f.fields.get('height')?.row.hasAttribute('data-sc-search-field-hidden')).toBe(false);
});

test('pending commands and Node context restore with query, active match and scroll intact', () => {
  const f = fixture();
  f.search('@node latent "512"');
  f.click('Next match (Enter)');
  f.root.scrollTop = 123;
  const state = f.presenter.state;
  f.presenter.dispose();
  const restored = new MasonrySearchPresenter({
    root: f.root,
    container: f.container,
    surface: f.surface,
    state,
    setCards: f.setCards,
  });
  presenters.push(restored);
  expect(restored.state).toEqual(state);
  expect(f.root.scrollTop).toBe(123);
  expect(f.container.querySelector<HTMLElement>('.sc-masonry-finder')?.hidden).toBe(false);
  restored.close(false);
  const closed = restored.state;
  restored.dispose();
  const reopened = new MasonrySearchPresenter({
    root: f.root,
    container: f.container,
    surface: f.surface,
    state: closed,
    setCards: f.setCards,
  });
  presenters.push(reopened);
  expect(reopened.state.open).toBe(false);
  reopened.open();
  expect(reopened.state.text).toBe('latent "512"');
});

test('source edits preserve native cursors and markup-like matches remain literal decorations', () => {
  const f = fixture();
  const control = f.fields.get('width')?.control;
  if (!control) throw new Error('Native control missing');
  control.value = '<img onerror=boom>512';
  f.presenter.valuesChanged();
  f.search('<img');
  expect(f.root.querySelector('img')).toBeNull();
  expect(f.root.querySelector('mark')?.textContent).toBe('<img');
  control.focus();
  control.setSelectionRange(2, 2);
  f.presenter.valuesChanged();
  expect(document.activeElement).toBe(control);
  expect(control.selectionStart).toBe(2);
  f.input.control.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, composed: true }),
  );
  expect(f.presenter.state.open).toBe(false);
  expect(document.activeElement).toBe(control);
});

test('query updates retain decoration identity and cleanup restores rows removed from the current corpus', () => {
  const f = fixture();
  f.search('51');
  const decoration = f.fields.get('width')?.row.querySelector('.sc-search-highlight');
  const styles = jest.spyOn(window, 'getComputedStyle');
  styles.mockClear();
  f.search('512');
  expect(styles).not.toHaveBeenCalled();
  expect(f.fields.get('width')?.row.querySelector('.sc-search-highlight')).toBe(decoration);
  const visibility = new SearchVisibility(f.fields, f.setCards);
  visibility.apply(matchSearch(f.surface.read(), buildSearchQuery('field', 'width')));
  const row = f.fields.get('height')?.row;
  f.fields.delete('height');
  visibility.dispose();
  expect(row?.hasAttribute('data-sc-search-field-hidden')).toBe(false);
});
