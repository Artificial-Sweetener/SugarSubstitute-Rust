import { cubeFacePresentedWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
import { nativeNodeFieldRows } from '../../surface/NativeNodeFieldRows.js';
import { formatDisplayLabel } from '../../surface/appearance/DisplayLabels.js';
/** Retain native references only while mounted; query and navigation never rediscover the graph. */
export class MasonrySearchCorpus {
    fields = new Map();
    #cards = new Map();
    #nodes = new Map();
    #rowCards = new WeakMap();
    /** Convert policy-visible cards and forwarded controls in authoritative projection order. */
    constructor(sections, cells) {
        for (const section of sections)
            for (const card of section.cards) {
                const cell = cells.get(card.id);
                if (card.visible && cell)
                    this.#cards.set(card.id, { card, cell });
            }
        this.refreshCards(new Set(this.#cards.keys()));
    }
    /** Resolve mounted or recently removed native rows without inspecting arbitrary popup ancestors. */
    cardFor(row) {
        const id = row.closest('[data-masonry-card]')?.dataset.masonryCard ??
            this.#rowCards.get(row);
        return id && this.#cards.has(id) ? id : null;
    }
    /** Rebind exactly the cards identified by native row-mount changes. */
    refreshCards(ids) {
        for (const id of ids) {
            const mounted = this.#cards.get(id);
            if (!mounted)
                continue;
            for (const field of this.#nodes.get(id)?.fields ?? [])
                this.fields.delete(field.description.id);
            const { card, cell } = mounted;
            const rows = nativeNodeFieldRows(cell);
            const widgets = new Map(cubeFacePresentedWidgets(card.node).map((widget) => [widget.name, widget]));
            for (const source of card.widgetSources ?? []) {
                const widget = source.node.widgets?.find((candidate) => candidate.name === source.name);
                if (widget)
                    widgets.set(source.targetName, widget);
            }
            const fields = [];
            for (const [key, widget] of widgets) {
                const row = rows.get(key);
                if (!row || row.hidden || row.getAttribute('aria-hidden') === 'true')
                    continue;
                const fieldId = JSON.stringify([card.id, key]);
                const label = row
                    .querySelector('[data-testid="widget-layout-field-label"]')
                    ?.textContent?.trim() ||
                    (typeof widget.options?.label === 'string'
                        ? widget.options.label
                        : formatDisplayLabel(key));
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
    read() {
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
function searchValue(value) {
    if (typeof value === 'boolean')
        return value ? 'True' : 'False';
    return typeof value === 'string' || typeof value === 'number' ? String(value) : null;
}
/** Accept text-selectable native controls while excluding combo-popup search inputs. */
function selectableControl(row) {
    const textarea = row.querySelector('textarea');
    if (textarea)
        return textarea;
    const input = row.querySelector('input:not([role="combobox"])');
    return input instanceof HTMLInputElement &&
        ['text', 'search', 'tel', 'url', 'password'].includes(input.type)
        ? input
        : null;
}
