import { prepareNativePresentation } from '../../surface/NativePresentationReady.js';
/** Keep row masks separate from the masonry owner's card packing and visibility. */
export class SearchVisibility {
    #fields;
    #setCards;
    #hidden = new Set();
    /** Bind only the identified native fields and the authoritative layout owner. */
    constructor(fields, setCards) {
        this.#fields = fields;
        this.#setCards = setCards;
    }
    /** Hide unmatched rows in Field mode and unmatched cards in Field or Node mode. */
    apply(result) {
        const filter = result && result.query.normalized ? result : null;
        const fieldFilter = filter?.query.mode === 'field' ? filter : null;
        const changed = new Set();
        if (!fieldFilter) {
            for (const row of this.#hidden)
                row.removeAttribute('data-sc-search-field-hidden');
            for (const field of this.#fields.values())
                if (this.#hidden.has(field.row))
                    changed.add(field.card);
            this.#hidden.clear();
        }
        for (const field of this.#fields.values()) {
            const hidden = fieldFilter !== null && !fieldFilter.fields.has(field.id);
            if (hidden === this.#hidden.has(field.row))
                continue;
            if (hidden) {
                field.row.setAttribute('data-sc-search-field-hidden', '');
                this.#hidden.add(field.row);
            }
            else {
                field.row.removeAttribute('data-sc-search-field-hidden');
                this.#hidden.delete(field.row);
            }
            changed.add(field.card);
        }
        for (const card of changed)
            prepareNativePresentation(card);
        this.#setCards(filter && filter.query.mode !== 'text' ? filter.nodes : null, changed.size > 0);
    }
    /** Restore the native row presentation before returning borrowed controls to the graph. */
    dispose() {
        this.apply(null);
    }
}
