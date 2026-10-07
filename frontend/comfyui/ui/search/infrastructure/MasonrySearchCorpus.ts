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
/** Adapt only already-projected masonry nodes and identified native rows into a finder corpus. */
import type { ComfyWidget } from '../../types/graph.js';
import type {
  WorkflowMasonrySection,
  WorkflowMasonryCard,
} from '../../surface/masonry/WorkflowMasonryProjection.js';
import { cubeFacePresentedWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
import { nativeNodeFieldRows } from '../../surface/NativeNodeFieldRows.js';
import { formatFluentDisplayLabel } from '../../surface/appearance/FluentDisplayLabels.js';
import type { SearchField, SearchNode } from '../domain/SearchMatching.js';
import type {
  SearchFieldSurface,
  SearchSurface,
  SearchTextControl,
} from '../presentation/SearchSurface.js';

interface SourceField {
  readonly description: Omit<SearchField, 'text'>;
  readonly widget: ComfyWidget;
}
interface SourceNode {
  readonly description: Omit<SearchNode, 'fields'>;
  readonly fields: readonly SourceField[];
}
/** Retain native references only while mounted; query and navigation never rediscover the graph. */
export class MasonrySearchCorpus implements SearchSurface {
  readonly fields = new Map<string, SearchFieldSurface>();
  readonly #cards = new Map<string, { card: WorkflowMasonryCard; cell: HTMLElement }>();
  readonly #nodes = new Map<string, SourceNode>();
  readonly #rowCards = new WeakMap<HTMLElement, string>();

  /** Convert policy-visible cards and forwarded controls in authoritative projection order. */
  constructor(
    sections: readonly WorkflowMasonrySection[],
    cells: ReadonlyMap<string, HTMLElement>,
  ) {
    for (const section of sections)
      for (const card of section.cards) {
        const cell = cells.get(card.id);
        if (card.visible && cell) this.#cards.set(card.id, { card, cell });
      }
    this.refreshCards(new Set(this.#cards.keys()));
  }
  /** Resolve mounted or recently removed native rows without inspecting arbitrary popup ancestors. */
  cardFor(row: HTMLElement): string | null {
    const id =
      row.closest<HTMLElement>('[data-masonry-card]')?.dataset.masonryCard ??
      this.#rowCards.get(row);
    return id && this.#cards.has(id) ? id : null;
  }
  /** Rebind exactly the cards identified by native row-mount changes. */
  refreshCards(ids: ReadonlySet<string>): void {
    for (const id of ids) {
      const mounted = this.#cards.get(id);
      if (!mounted) continue;
      for (const field of this.#nodes.get(id)?.fields ?? [])
        this.fields.delete(field.description.id);
      const { card, cell } = mounted;
      const rows = nativeNodeFieldRows(cell);
      const widgets = new Map(
        cubeFacePresentedWidgets(card.node).map((widget) => [widget.name, widget]),
      );
      for (const source of card.widgetSources ?? []) {
        const widget = source.node.widgets?.find((candidate) => candidate.name === source.name);
        if (widget) widgets.set(source.targetName, widget);
      }
      const fields: SourceField[] = [];
      for (const [key, widget] of widgets) {
        const row = rows.get(key);
        if (!row || row.hidden || row.getAttribute('aria-hidden') === 'true') continue;
        const fieldId = JSON.stringify([card.id, key]);
        const label =
          row
            .querySelector<HTMLElement>('[data-testid="widget-layout-field-label"]')
            ?.textContent?.trim() ||
          (typeof widget.options?.label === 'string'
            ? widget.options.label
            : formatFluentDisplayLabel(key));
        fields.push({ description: { id: fieldId, key, label }, widget });
        this.fields.set(fieldId, {
          id: fieldId,
          node: card.id,
          card: cell,
          row,
          control: selectableControl(row),
        });
        this.#rowCards.set(row, card.id);
      }
      this.#nodes.set(id, {
        description: {
          id: card.id,
          name: card.label,
          type: card.node.type ?? card.node.class_type ?? '',
          title: card.node.title ?? card.label,
        },
        fields,
      });
    }
  }
  /** Snapshot primitive values without reading popup content or rebuilding mounted lookup. */
  read(): readonly SearchNode[] {
    return [...this.#cards.keys()].flatMap((id) => {
      const node = this.#nodes.get(id);
      return node
        ? [
            {
              ...node.description,
              fields: node.fields.map((field) => ({
                ...field.description,
                text: searchValue(field.widget.value),
              })),
            },
          ]
        : [];
    });
  }
}
/** Preserve literal scalar semantics and exclude arbitrary structured widget payloads. */
function searchValue(value: unknown): string | null {
  if (typeof value === 'boolean') return value ? 'True' : 'False';
  return typeof value === 'string' || typeof value === 'number' ? String(value) : null;
}
/** Accept text-selectable native controls while excluding combo-popup search inputs. */
function selectableControl(row: HTMLElement): SearchTextControl | null {
  const textarea = row.querySelector('textarea');
  if (textarea) return textarea;
  const input = row.querySelector('input:not([role="combobox"])');
  return input instanceof HTMLInputElement &&
    ['text', 'search', 'tel', 'url', 'password'].includes(input.type)
    ? input
    : null;
}
