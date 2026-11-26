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
/**
 * Show the finder's matches in text outside prompt editors, through the shared search highlights.
 * A value a card renders as plain text carries ranges of the shared highlights, as a prompt editor's
 * text does. A text box draws its own value where no range can reach, and paints its selection as
 * tall as its line: a transparent copy of its value lies over it, with each match a run of that
 * copy filled to the line and styled by the same rule as the highlights.
 */
import { findSearchRanges } from '../domain/SearchMatching.js';
import { SearchHighlightRanges } from './SearchHighlightRanges.js';
const typography = [
    'font-family',
    'font-size',
    'font-weight',
    'font-style',
    'font-variant',
    'line-height',
    'letter-spacing',
    'word-spacing',
    'text-align',
    'text-indent',
    'text-transform',
    'direction',
    'tab-size',
    'padding-top',
    'padding-right',
    'padding-bottom',
    'padding-left',
    'border-top-width',
    'border-right-width',
    'border-bottom-width',
    'border-left-width',
    'border-radius',
    'word-break',
    'overflow-wrap',
];
/** Own the text-box copies and every range outside prompt editors; release all on teardown. */
export class SearchHighlights {
    #bindings = new Map();
    #ranges;
    #observer;
    #document;
    #frame = null;
    #shown = null;
    /** Observe matched native geometry without discovering graph candidates or following animation frames. */
    constructor(documentRef) {
        this.#document = documentRef;
        this.#ranges = new SearchHighlightRanges(documentRef.defaultView);
        const Observer = documentRef.defaultView?.ResizeObserver;
        this.#observer = Observer ? new Observer(() => this.#schedule()) : null;
    }
    /**
     * Show every match of the searched text, with `active` as the current one.
     * - A text box is searched in the text it displays, so a number shown with more digits than its
     *   value holds, or a value whose line endings differ, is marked on its own characters.
     * - A value a card renders as plain text, such as a chosen sampler, is marked where it stands.
     * A Field or Node query narrows what is shown and marks nothing.
     */
    render(surface, result, active) {
        this.#shown = { surface, result };
        const needle = result.query.text;
        const grouped = new Map();
        if (needle)
            for (const match of result.matches) {
                const matches = grouped.get(match.field) ?? [];
                matches.push(match);
                grouped.set(match.field, matches);
            }
        for (const [id, binding] of this.#bindings)
            if (!grouped.has(id) || surface.fields.get(id)?.control !== binding.control)
                this.#release(id, binding);
        const created = [];
        const passive = [];
        for (const [id, matches] of grouped) {
            const control = surface.fields.get(id)?.control;
            if (!control)
                continue;
            const binding = this.#bindings.get(id) ?? this.#bind(id, control, created);
            if (!binding)
                continue;
            const shown = control.value;
            const occurrences = findSearchRanges(shown, needle);
            const ordinal = active?.field === id ? matches.indexOf(active) : -1;
            const chosen = ordinal < 0
                ? undefined
                : occurrences.length === matches.length
                    ? occurrences[ordinal]
                    : occurrences.find((occurrence) => occurrence.start === active?.start);
            const signature = JSON.stringify([shown, needle, chosen?.start ?? -1]);
            if (binding.layer.dataset.shown === signature)
                continue;
            binding.layer.dataset.shown = signature;
            binding.current = null;
            const parts = [];
            let offset = 0;
            for (const occurrence of occurrences) {
                parts.push(this.#document.createTextNode(shown.slice(offset, occurrence.start)));
                const run = this.#document.createElement('span');
                run.dataset.scSearch = occurrence === chosen ? 'current' : 'match';
                run.textContent = shown.slice(occurrence.start, occurrence.start + occurrence.length);
                if (occurrence === chosen)
                    binding.current = run;
                parts.push(run);
                offset = occurrence.start + occurrence.length;
            }
            // A trailing zero-width space keeps a final empty line as tall as the box draws it.
            parts.push(this.#document.createTextNode(`${shown.slice(offset)}\u200b`));
            binding.text.replaceChildren(...parts);
            if (!binding.filled && !created.includes(binding))
                this.#fill(binding);
        }
        if (needle)
            for (const field of surface.fields.values())
                if (!field.control && !field.painter && this.#searched(field, result))
                    this.#mark(passive, field.row, needle, field.label);
        if (created.length)
            this.#geometry(created);
        this.#ranges.replace(passive, []);
    }
    /** Move the current match without searching again. */
    activate(active) {
        if (this.#shown)
            this.render(this.#shown.surface, this.#shown.result, active);
    }
    /** Scroll a text box to its current match; false when the match is not in a text box. */
    reveal(match) {
        const binding = this.#bindings.get(match.field);
        if (!binding)
            return false;
        const { control, current } = binding;
        if (!current)
            return true;
        const fragment = current.getBoundingClientRect();
        const bounds = control.getBoundingClientRect();
        if (fragment.top < bounds.top)
            control.scrollTop -= bounds.top - fragment.top + 4;
        else if (fragment.bottom > bounds.bottom)
            control.scrollTop += fragment.bottom - bounds.bottom + 4;
        if (fragment.left < bounds.left)
            control.scrollLeft -= bounds.left - fragment.left + 4;
        else if (fragment.right > bounds.right && control instanceof HTMLInputElement)
            control.scrollLeft += fragment.right - bounds.right + 4;
        binding.scroll();
        return true;
    }
    /** Remove every range and copy without moving native editing cursors. */
    clear() {
        this.#shown = null;
        this.#ranges.clear();
        for (const [id, binding] of this.#bindings)
            this.#release(id, binding);
    }
    /** Stop all owned callbacks before borrowed native controls return to graph mode. */
    dispose() {
        if (this.#frame !== null)
            this.#document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.clear();
        this.#observer?.disconnect();
    }
    /** Whether a field's shown value is among the text this query searches. */
    #searched(field, result) {
        if (field.row.hidden || field.row.hasAttribute('data-sc-search-field-hidden'))
            return false;
        const { query } = result;
        return query.mode === 'text' || !query.node || result.nodes.has(field.node);
    }
    /** Collect the needle's occurrences in the text shown under `root`, outside `except`. */
    #mark(ranges, root, needle, except) {
        const walker = this.#document.createTreeWalker(root, 4);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
            const parent = node.parentElement;
            if (!parent || except?.contains(parent) || parent.closest('.sc-search-highlight'))
                continue;
            for (const found of findSearchRanges(node.nodeValue ?? '', needle)) {
                const range = this.#document.createRange();
                range.setStart(node, found.start);
                range.setEnd(node, found.start + found.length);
                ranges.push(range);
            }
        }
    }
    /** Lay one transparent copy over a text box, in the box's parent, following its scrolling. */
    #bind(id, control, created) {
        const parent = control.parentElement;
        if (!parent)
            return null;
        const position = parent.style.getPropertyValue('position');
        const positionPriority = parent.style.getPropertyPriority('position');
        if (getComputedStyle(parent).position === 'static')
            parent.style.setProperty('position', 'relative');
        const layer = this.#document.createElement('div');
        layer.className = 'sc-search-highlight';
        layer.setAttribute('aria-hidden', 'true');
        const text = this.#document.createElement('div');
        text.className = 'sc-search-highlight__text';
        layer.append(text);
        const scroll = () => {
            text.style.transform = `translate(${-control.scrollLeft}px,${-control.scrollTop}px)`;
        };
        const binding = {
            control,
            parent,
            position,
            positionPriority,
            layer,
            text,
            scroll,
            current: null,
            filled: false,
        };
        parent.append(layer);
        control.addEventListener('scroll', scroll, { passive: true });
        this.#bindings.set(id, binding);
        this.#observer?.observe(control);
        created.push(binding);
        return binding;
    }
    /** Copy native typography and local dimensions so wrapped highlights share the real widget layout. */
    #geometry(bindings = this.#bindings.values()) {
        const measurements = [...bindings].map((binding) => {
            const { control } = binding;
            const style = getComputedStyle(control);
            return {
                binding,
                typography: typography.map((property) => [property, style.getPropertyValue(property)]),
                ink: style.color,
                left: control.offsetLeft,
                top: control.offsetTop,
                width: control.offsetWidth,
                height: control.offsetHeight,
                x: control.scrollLeft,
                y: control.scrollTop,
            };
        });
        for (const measurement of measurements) {
            const { control, layer, text } = measurement.binding;
            for (const [property, value] of measurement.typography)
                layer.style.setProperty(property, value);
            // The copy's own text is transparent; a highlighted run is drawn in the box's ink.
            layer.style.setProperty('--sc-search-mirror-ink', measurement.ink);
            layer.style.boxSizing = 'border-box';
            layer.style.borderStyle = 'solid';
            layer.style.borderColor = 'transparent';
            layer.style.left = `${measurement.left}px`;
            layer.style.top = `${measurement.top}px`;
            layer.style.width = `${measurement.width}px`;
            layer.style.height = `${measurement.height}px`;
            text.style.whiteSpace =
                control instanceof HTMLTextAreaElement && control.wrap !== 'off' ? 'pre-wrap' : 'pre';
            text.style.transform = `translate(${-measurement.x}px,${-measurement.y}px)`;
            this.#fill(measurement.binding);
        }
    }
    /**
     * A text box paints its selection as tall as its line, and a run of text only as tall as its
     * glyphs. Each marked run is padded by the difference, so it covers what a selection covers.
     * The box sets its glyphs a whole pixel from the line's top and leaves an odd pixel of the
     * difference below them, where block layout would split it in halves; the copy's text is raised
     * by that half pixel, and its runs padded around it, so its glyphs stand on the box's own.
     */
    #fill(binding) {
        const run = binding.text.querySelector('[data-sc-search]');
        if (!run)
            return;
        binding.filled = true;
        const { style } = binding.layer;
        for (const property of ['above', 'below', 'raise'])
            style.setProperty(`--sc-search-line-${property}`, '0px');
        const line = parseFloat(getComputedStyle(binding.control).lineHeight);
        const glyphs = run.getClientRects()[0]?.height ?? 0;
        if (!Number.isFinite(line) || glyphs <= 0 || line <= glyphs)
            return;
        const half = (line - glyphs) / 2;
        const above = Math.floor(half);
        style.setProperty('--sc-search-line-above', `${String(above)}px`);
        style.setProperty('--sc-search-line-below', `${String(line - glyphs - above)}px`);
        style.setProperty('--sc-search-line-raise', `${String(half - above)}px`);
    }
    /** Coalesce geometry changes independently of query matching and native field lookup. */
    #schedule() {
        if (this.#frame !== null)
            return;
        const windowRef = this.#document.defaultView;
        if (!windowRef)
            return;
        this.#frame = windowRef.requestAnimationFrame(() => {
            this.#frame = null;
            this.#geometry();
        });
    }
    /** Restore only presentation properties owned by this matched control. */
    #release(id, binding) {
        this.#observer?.unobserve(binding.control);
        binding.control.removeEventListener('scroll', binding.scroll);
        binding.layer.remove();
        if (binding.position)
            binding.parent.style.setProperty('position', binding.position, binding.positionPriority);
        else
            binding.parent.style.removeProperty('position');
        this.#bindings.delete(id);
    }
}
