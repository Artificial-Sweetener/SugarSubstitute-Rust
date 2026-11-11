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
/** Coordinate finder intent, transient filtering, native highlights and match navigation. */
import { SearchSession } from '../application/SearchSession.js';
import { FinderView } from './FinderView.js';
import { SearchHighlights } from './SearchHighlights.js';
import { SearchVisibility } from './SearchVisibility.js';
import { subscribeVisualStyle } from '../../surface/appearance/VisualStyleBoundary.js';
/** Own one mounted finder lifetime without storing native controls in portable workflow state. */
export class MasonrySearchPresenter {
    #options;
    #session;
    #finder;
    #stopStyle;
    #highlights;
    #visibility;
    #surface;
    #frame = null;
    #pending = null;
    #disposed = false;
    /** Restore portable intent and query location against this surface's current native controls. */
    constructor(options) {
        this.#options = options;
        this.#surface = options.surface;
        this.#session = new SearchSession(options.state);
        this.#visibility = new SearchVisibility(options.surface.fields, options.setCards);
        this.#highlights = new SearchHighlights(options.root.ownerDocument);
        this.#finder = this.#createFinder();
        this.#stopStyle = subscribeVisualStyle(options.root.ownerDocument, () => {
            this.#flush();
            this.#finder.dispose();
            this.#finder = this.#createFinder();
            this.#sync();
            if (this.#session.state.open)
                this.#finder.show(options.container, false);
        });
        this.#refresh();
        this.#sync();
        if (this.#session.state.open) {
            this.#finder.show(options.container, false);
            this.#render(false);
        }
        options.root.scrollTop = this.#session.state.scrollTop;
    }
    /** Rebind only native surface membership while retaining finder input, popup and focus identity. */
    updateSurface(surface) {
        this.#flush();
        this.#visibility.dispose();
        this.#surface = surface;
        this.#visibility = new SearchVisibility(surface.fields, this.#options.setCards);
        this.#refresh();
        if (this.#session.state.open)
            this.#render(false);
        else
            this.#sync();
    }
    /** Capture pending input and scroll position without retaining the native surface. */
    get state() {
        this.#flush();
        this.#session.setScrollTop(this.#options.root.scrollTop);
        return this.#session.state;
    }
    /** Open or refocus the finder and select the existing query just like Ctrl+F in Substitute. */
    open() {
        this.#session.setOpen(true);
        this.#refresh();
        this.#render(false);
        this.#finder.show(this.#options.container, true);
    }
    /** Clear transient search presentation and optionally focus the active native field on Escape. */
    close(focusMatch) {
        this.#flush();
        const active = this.#session.state.active;
        this.#session.setOpen(false);
        this.#visibility.apply(null);
        this.#highlights.clear();
        this.#finder.hide();
        for (const field of this.#surface.fields.values()) {
            const control = field.control;
            if (control?.selectionEnd !== null && control?.selectionEnd !== undefined)
                control.setSelectionRange(control.selectionEnd, control.selectionEnd);
        }
        if (focusMatch) {
            const control = active ? this.#surface.fields.get(active.field)?.control : null;
            if (control && active) {
                control.focus({ preventScroll: true });
                control.setSelectionRange(active.start, active.start + active.length);
            }
            else
                this.#options.root.focus({ preventScroll: true });
        }
    }
    /** Navigate already-computed selectable matches without rebuilding the source corpus. */
    cycle(direction) {
        this.#flush();
        if (!this.#session.state.open || this.#session.state.mode === 'field')
            return;
        const active = this.#session.cycle(direction);
        this.#highlights.activate(active);
        this.#sync();
        if (active)
            this.#reveal(active);
    }
    /** Refresh source values after native input without resetting native editing cursors. */
    valuesChanged() {
        if (!this.#session.state.open || this.#disposed)
            return;
        this.#refresh();
        this.#render(false);
    }
    /** Release every callback, highlight and filter before the renderer relinquishes this surface. */
    dispose() {
        if (this.#disposed)
            return;
        this.#flush();
        this.#disposed = true;
        this.#stopStyle();
        this.#visibility.dispose();
        this.#highlights.dispose();
        this.#finder.dispose();
    }
    /** Recreate provider controls while retaining the query, results, filters, and native field corpus. */
    #createFinder() {
        return new FinderView(this.#options.root.ownerDocument, {
            input: (mode, text) => this.#queue(mode, text),
            cycle: (direction) => this.cycle(direction),
            close: (focus) => this.close(focus),
        });
    }
    /** Coalesce rapid typing while keeping themed's native input response immediate. */
    #queue(mode, text) {
        this.#pending = { mode, text };
        if (this.#frame !== null)
            return;
        const windowRef = this.#options.root.ownerDocument.defaultView;
        if (!windowRef)
            return;
        this.#frame = windowRef.requestAnimationFrame(() => {
            this.#frame = null;
            this.#flush();
        });
    }
    /** Apply the last query before navigation or a surface switch so no keystroke is lost. */
    #flush() {
        if (this.#frame !== null)
            this.#options.root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        const pending = this.#pending;
        this.#pending = null;
        if (!pending || this.#disposed)
            return;
        const applied = this.#session.setInput(pending.mode, pending.text);
        this.#selectable();
        this.#sync();
        if (applied && this.#session.state.open)
            this.#render(true);
    }
    /** Read values only from the mounted corpus and retain closest active-field selection. */
    #refresh() {
        this.#session.setCorpus(this.#surface.read());
        this.#selectable();
    }
    /** Keep unsupported combo controls out of text navigation while retaining semantic filtering. */
    #selectable() {
        this.#session.selectable(new Set([...this.#surface.fields.values()]
            .filter((field) => field.control !== null)
            .map((field) => field.id)));
    }
    /** Paint results independently of source values and native field ownership. */
    #render(reveal) {
        this.#visibility.apply(this.#session.result);
        this.#highlights.render(this.#surface.fields, this.#session.result.matches, this.#session.state.active);
        this.#sync();
        if (reveal && this.#session.state.active)
            this.#reveal(this.#session.state.active);
    }
    /** Update reference chrome and accessible navigation status without changing popup identity. */
    #sync() {
        const state = this.#session.state;
        this.#finder.sync(state.mode, state.text, this.#session.result.matches.length, this.#session.index, !state.query.normalized
            ? 'Search ready'
            : state.mode === 'field'
                ? `${this.#session.result.fields.size} matching fields`
                : state.mode === 'node' && !state.query.text
                    ? `${this.#session.result.nodes.size} matching nodes`
                    : 'No text matches');
    }
    /** Select inside the native widget and scroll only when the active match lies outside the viewport. */
    #reveal(match) {
        const field = this.#surface.fields.get(match.field);
        const control = field?.control;
        if (!control)
            return;
        control.setSelectionRange(match.start, match.start + match.length);
        this.#highlights.reveal(match);
        const root = this.#options.root;
        const bounds = root.getBoundingClientRect();
        const target = control.getBoundingClientRect();
        if (target.top < bounds.top + 64 || target.bottom > bounds.bottom - 16)
            root.scrollTo?.({
                top: Math.max(0, root.scrollTop + target.top - bounds.top - Math.min(96, bounds.height / 3)),
                behavior: 'smooth',
            });
    }
}
