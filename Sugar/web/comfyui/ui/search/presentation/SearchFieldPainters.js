/** Own which prompts currently show matches, so each is cleared exactly once. */
export class SearchFieldPainters {
    #painted = new Map();
    #matches = [];
    #query = '';
    /** Paint every prompt's matches for `query` and clear prompts that no longer match. */
    render(fields, matches, active, query) {
        this.#matches = matches;
        this.#query = query;
        const matched = new Set(matches.map((match) => match.field));
        for (const [id, painter] of this.#painted)
            if (!matched.has(id) || fields.get(id)?.painter !== painter) {
                painter.clear();
                this.#painted.delete(id);
            }
        for (const id of matched) {
            const painter = fields.get(id)?.painter;
            if (!painter)
                continue;
            this.#painted.set(id, painter);
            this.#paint(id, painter, active);
        }
    }
    /** Move the stronger current-match colour between prompts without recomputing matches. */
    activate(previous, active) {
        for (const id of new Set([previous?.field, active?.field])) {
            const painter = id === undefined ? undefined : this.#painted.get(id);
            if (id !== undefined && painter)
                this.#paint(id, painter, active);
        }
    }
    /** Select and scroll to one match inside its prompt; false when another renderer owns it. */
    reveal(match) {
        const painter = this.#painted.get(match.field);
        if (!painter)
            return false;
        painter.reveal(this.#index(match));
        return true;
    }
    /**
     * Remove every prompt's matches. With `focus`, the prompt holding that match first takes
     * focus with the match selected; the return says whether a prompt did.
     */
    clear(focus = null) {
        const focused = focus ? this.#painted.get(focus.field) : undefined;
        if (focus && focused)
            focused.focus(this.#index(focus));
        for (const painter of this.#painted.values())
            if (painter !== focused)
                painter.clear();
        this.#painted.clear();
        this.#matches = [];
        return focused !== undefined;
    }
    #within(field) {
        return this.#matches.filter((match) => match.field === field);
    }
    /** Where a match stands among its own field's matches. */
    #index(match) {
        return this.#within(match.field).findIndex((candidate) => candidate.start === match.start && candidate.length === match.length);
    }
    #paint(field, painter, active) {
        const index = active?.field === field ? this.#index(active) : -1;
        painter.paint(this.#query, this.#within(field), index < 0 ? null : index);
    }
}
