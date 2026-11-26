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
 * The settings workspace as the Python application presents it. In the SugarSubstitute frame it
 * is the whole settings dialog: the navigation pane (the SugarSubstitute pages, then ComfyUI's
 * own settings and panels, then other extensions'), the toolbar search, and one page at a time
 * under a "Settings › Page" header. On a plain ComfyUI page it is SugarCubes' page: its pages
 * as segmented tabs with a search field, and the chosen page below.
 */
import { NavigationRail } from './NavigationRail.js';
import { createFluentIcon } from '../../core/FluentIcons.js';
import { SegmentedSlider } from '../../core/SegmentedSlider.js';
import { applyOwnedTheme } from '../../surface/appearance/ThemeService.js';
import { renderSettingsRow } from './SettingsCards.js';
import { searchSettings } from './SettingsSearch.js';
const NARROW = 286;
const SEARCH_DEBOUNCE_MS = 125;
/** One workspace, in the frame or on a page. */
export class SettingsWorkspaceView {
    document;
    report;
    frame;
    element;
    #catalog;
    #content;
    #header;
    #nav;
    #rail;
    #tabs;
    #search;
    #cards = [];
    #sectionStops = [];
    #observer;
    #stops = [];
    #page;
    #query = '';
    #searchTimer = null;
    #rendered = '';
    #queued = false;
    constructor(document, host, catalog, report, frame = null) {
        this.document = document;
        this.report = report;
        this.frame = frame;
        this.#catalog = catalog;
        this.#page = catalog.pages()[0]?.id ?? 'about';
        this.element = document.createElement('div');
        this.element.className = 'sc-sw';
        this.element.dataset.mode = host;
        this.element.dataset.look = 'comfy';
        this.element.dataset.scheme = 'dark';
        this.#content = document.createElement('div');
        this.#content.className = 'sc-sw-page';
        this.#search = document.createElement('input');
        this.#search.type = 'search';
        this.#search.className = 'sc-sw-field';
        this.#search.placeholder = 'Search settings';
        this.#search.setAttribute('aria-label', 'Search settings');
        this.#search.addEventListener('input', () => {
            if (this.#searchTimer)
                clearTimeout(this.#searchTimer);
            this.#searchTimer = setTimeout(() => {
                this.#searchTimer = null;
                this.#query = this.#search.value.trim();
                this.#render();
            }, SEARCH_DEBOUNCE_MS);
        });
        const search = document.createElement('div');
        search.className = 'sc-sw-search';
        const glass = createFluentIcon(document, 'search', 14);
        glass.classList.add('sc-sw-search-icon');
        search.append(this.#search, glass);
        if (host === 'frame') {
            const toolbar = document.createElement('div');
            toolbar.className = 'sc-sw-toolbar';
            const close = document.createElement('button');
            close.type = 'button';
            close.className = 'sc-sw-close';
            close.setAttribute('aria-label', 'Close settings');
            close.append(createFluentIcon(document, 'dismiss', 16));
            close.addEventListener('click', () => this.frame?.close());
            toolbar.append(search, close);
            this.#nav = document.createElement('nav');
            this.#nav.className = 'sc-sw-nav';
            this.#nav.setAttribute('aria-label', 'Settings pages');
            this.#rail = new NavigationRail(document);
            const main = document.createElement('main');
            main.className = 'sc-sw-main';
            const column = document.createElement('div');
            column.className = 'sc-sw-column';
            this.#header = document.createElement('h1');
            this.#header.className = 'sc-sw-header';
            column.append(this.#header, this.#content);
            main.append(column);
            this.element.append(toolbar, this.#nav, main);
            this.#tabs = null;
        }
        else {
            this.#nav = null;
            this.#rail = null;
            this.#header = null;
            this.#tabs = new SegmentedSlider(document, 'SugarCubes settings pages', (id) => this.open(id));
            const strip = document.createElement('div');
            strip.className = 'sc-sw-tabs-strip';
            strip.append(this.#tabs.element);
            const bar = document.createElement('div');
            bar.className = 'sc-sw-page-bar';
            bar.append(strip, search);
            this.element.append(bar, this.#content);
        }
        const Observer = document.defaultView?.ResizeObserver;
        this.#observer = Observer ? new Observer((entries) => this.#wrap(entries)) : null;
        this.#stops.push(catalog.subscribe(this.#queue));
        this.#render();
    }
    /** Follow the active visual style and palette. */
    setAppearance(style, dark) {
        if (this.element.dataset.look !== style)
            this.element.dataset.look = style;
        const scheme = dark ? 'dark' : 'light';
        if (this.element.dataset.scheme !== scheme)
            this.element.dataset.scheme = scheme;
        applyOwnedTheme(this.element, style === 'comfy' ? null : dark, style === 'comfy' ? undefined : style);
    }
    /** Open one page (or one of ComfyUI's panels in the frame), clearing any search. */
    open(id) {
        const previous = this.#entries().findIndex((entry) => entry.id === this.#page);
        this.#page = id;
        this.#query = '';
        this.#search.value = '';
        const next = this.#entries().findIndex((entry) => entry.id === id);
        this.element.style.setProperty('--sc-sw-enter-from', next >= previous ? '24px' : '-24px');
        this.#render(true);
    }
    /** Re-read ComfyUI's pages and panels, as when the dialog opens. */
    refresh() {
        this.#render();
    }
    /** Focus the search field, as the Python toolbar search takes typing at once. */
    focusSearch() {
        this.#search.focus();
    }
    dispose() {
        if (this.#searchTimer)
            clearTimeout(this.#searchTimer);
        for (const stop of this.#stops.splice(0))
            stop();
        this.#clearCards();
        this.#observer?.disconnect();
        this.#tabs?.dispose();
        this.frame?.showPanel(null);
        this.element.remove();
    }
    #queue = () => {
        if (this.#queued)
            return;
        this.#queued = true;
        queueMicrotask(() => {
            this.#queued = false;
            this.#render();
        });
    };
    /** Pages and panels in navigation order. */
    #entries() {
        const ours = this.#catalog.pages().map((page) => ({
            id: page.id,
            title: page.title,
            subtitle: page.subtitle,
            icon: page.icon,
            // Our pages are headed "Sugar" as ComfyUI's are headed "ComfyUI".
            group: 'Sugar',
            panel: false,
        }));
        if (!this.frame)
            return ours;
        const comfy = this.frame.comfyPages();
        const panels = this.frame.comfyPanels();
        return [
            ...ours,
            ...comfy
                .filter((page) => page.core)
                .map((page) => ({
                id: page.id,
                title: page.title,
                subtitle: '',
                icon: page.icon,
                group: 'ComfyUI',
                panel: false,
            })),
            ...panels.map((panel) => ({
                id: panel.id,
                title: panel.title,
                subtitle: '',
                ...(panel.icon ? { icon: panel.icon } : {}),
                group: 'ComfyUI',
                panel: true,
            })),
            ...comfy
                .filter((page) => !page.core)
                .map((page) => ({
                id: page.id,
                title: page.title,
                subtitle: '',
                icon: page.icon,
                group: 'ComfyUI extensions',
                panel: false,
            })),
        ];
    }
    #pages() {
        return [...this.#catalog.pages(), ...(this.frame?.comfyPages() ?? [])];
    }
    #render(entering = false) {
        const entries = this.#entries();
        if (!entries.some((entry) => entry.id === this.#page))
            this.#page = entries[0]?.id ?? '';
        const current = entries.find((entry) => entry.id === this.#page);
        this.#renderNavigation(entries);
        const panel = current?.panel === true && !this.#query;
        this.element.toggleAttribute('data-panel', panel);
        this.frame?.showPanel(panel ? this.#page : null);
        if (panel) {
            this.#clearCards();
            this.#rendered = `panel:${this.#page}`;
            return;
        }
        const pages = this.#pages();
        if (this.#query) {
            if (this.#header)
                this.#header.textContent = 'Settings › Search settings';
            this.#renderSearch(pages);
            return;
        }
        const page = pages.find((candidate) => candidate.id === this.#page);
        if (this.#header)
            this.#header.textContent = `Settings › ${page?.title ?? ''}`;
        const signature = `page:${this.#page}:${page ? page.sections.map((s) => s.rows.map((r) => r.id).join(',')).join('|') : ''}`;
        if (signature === this.#rendered && !entering)
            return;
        this.#rendered = signature;
        this.#clearCards();
        if (!page)
            return;
        for (const section of page.sections) {
            const block = this.document.createElement('section');
            block.className = 'sc-sw-section';
            if (section.title) {
                const title = this.document.createElement('h2');
                title.className = 'sc-sw-section-title';
                title.textContent = section.title;
                block.append(title);
            }
            if (section.subtitle) {
                const subtitle = this.document.createElement('p');
                subtitle.className = 'sc-sw-section-subtitle';
                subtitle.textContent = section.subtitle;
                block.append(subtitle);
            }
            const cards = this.document.createElement('div');
            cards.className = 'sc-sw-cards';
            for (const row of section.rows)
                cards.append(this.#card(renderSettingsRow(this.document, row, this.report)));
            block.append(cards);
            // As in Python, a section with no visible rows disappears with its title.
            const show = () => {
                block.hidden = section.rows.every((row) => row.visible?.read() === false);
            };
            show();
            for (const row of section.rows)
                if (row.visible)
                    this.#sectionStops.push(row.visible.subscribe(show));
            this.#content.append(block);
        }
        if (entering) {
            this.#content.setAttribute('data-entering', '');
            this.#content.addEventListener('animationend', () => this.#content.removeAttribute('data-entering'), {
                once: true,
            });
        }
    }
    /** Python's search page: a "Page › Section" line, then the row as a card leading to its page. */
    #renderSearch(pages) {
        this.#rendered = `search:${this.#query}`;
        this.#clearCards();
        const results = searchSettings(pages, this.#query);
        const list = this.document.createElement('div');
        list.className = 'sc-sw-cards';
        if (!results.length) {
            const empty = this.document.createElement('div');
            empty.className = 'sc-sw-empty';
            const title = this.document.createElement('div');
            title.className = 'sc-sw-card-title';
            title.textContent = 'No settings found';
            const description = this.document.createElement('div');
            description.className = 'sc-sw-card-description';
            description.textContent = 'Try a different search term.';
            empty.append(title, description);
            list.append(empty);
        }
        for (const result of results) {
            const crumb = this.document.createElement('div');
            crumb.className = 'sc-sw-crumb';
            crumb.textContent = `${result.page.title} › ${result.section.title}`;
            const view = renderSettingsRow(this.document, {
                id: result.row.id,
                title: result.row.title,
                ...(result.row.description ? { description: result.row.description } : {}),
            }, this.report);
            view.element.classList.add('sc-sw-interactive');
            const head = view.element.querySelector('.sc-sw-card-head');
            const chevron = this.document.createElement('span');
            chevron.className = 'sc-sw-chevron';
            chevron.append(createFluentIcon(this.document, 'chevron-right', 13));
            head?.append(chevron);
            head?.addEventListener('click', () => this.open(result.page.id));
            list.append(crumb, this.#card(view));
        }
        this.#content.append(list);
    }
    #renderNavigation(entries) {
        if (this.#tabs) {
            this.#tabs.update(entries.map((entry) => ({ id: entry.id, label: entry.title, title: entry.subtitle })), this.#query ? '' : this.#page);
            return;
        }
        const nav = this.#nav;
        if (!nav)
            return;
        const ids = entries.map((entry) => `${entry.group}/${entry.id}`).join('\n');
        if (nav.dataset.entries !== ids) {
            nav.dataset.entries = ids;
            nav.replaceChildren();
            let group = '';
            for (const entry of entries) {
                if (entry.group !== group) {
                    group = entry.group;
                    const heading = this.document.createElement('div');
                    heading.className = 'sc-sw-nav-group';
                    heading.textContent = group;
                    nav.append(heading);
                }
                const item = this.document.createElement('button');
                item.type = 'button';
                item.className = 'sc-sw-nav-item';
                item.dataset.entry = entry.id;
                if (entry.subtitle)
                    item.title = entry.subtitle;
                const icon = this.document.createElement('span');
                icon.className = 'sc-sw-nav-icon';
                if (entry.icon)
                    icon.append(typeof entry.icon === 'function'
                        ? entry.icon(this.document)
                        : createFluentIcon(this.document, entry.icon, 18));
                const label = this.document.createElement('span');
                label.textContent = entry.title;
                item.append(icon, label);
                // Choosing the open page again keeps a search, as in Python.
                item.addEventListener('click', () => {
                    if (entry.id !== this.#page)
                        this.open(entry.id);
                });
                nav.append(item);
            }
            if (this.#rail)
                nav.append(this.#rail.element);
        }
        let selected = null;
        for (const item of nav.querySelectorAll('.sc-sw-nav-item')) {
            const on = item.dataset.entry === this.#page;
            if (on) {
                item.setAttribute('aria-current', 'page');
                selected = item;
            }
            else
                item.removeAttribute('aria-current');
        }
        this.#rail?.place(selected);
    }
    #card(view) {
        this.#cards.push(view);
        this.#observer?.observe(view.element);
        for (const row of view.element.querySelectorAll('.sc-sw-row'))
            this.#observer?.observe(row);
        return view.element;
    }
    #clearCards() {
        for (const view of this.#cards.splice(0))
            view.dispose();
        for (const stop of this.#sectionStops.splice(0))
            stop();
        this.#observer?.disconnect();
        this.#content.replaceChildren();
    }
    /**
     * Python's wrap modes: controls under the text below the row's wrap width; no icon when narrow.
     *
     * Wrapping changes a card's height, and a taller page can bring the scroll bar that narrows
     * every card, so the change is made in the next frame and only when it is one. Made in the
     * observer's own delivery it would ask for another delivery in the same frame, which the
     * browser reports as a ResizeObserver loop.
     */
    #wrap(entries) {
        const changes = entries.flatMap((entry) => {
            const element = entry.target;
            if (!(element instanceof HTMLElement))
                return [];
            const width = entry.contentRect.width;
            const wrapped = width < Number(element.dataset.wrap ?? 476);
            const narrow = width < NARROW;
            return element.hasAttribute('data-wrapped') !== wrapped ||
                element.hasAttribute('data-narrow') !== narrow
                ? [{ element, wrapped, narrow }]
                : [];
        });
        if (!changes.length)
            return;
        const apply = () => {
            for (const { element, wrapped, narrow } of changes) {
                element.toggleAttribute('data-wrapped', wrapped);
                element.toggleAttribute('data-narrow', narrow);
            }
        };
        const frame = this.document.defaultView?.requestAnimationFrame;
        if (frame)
            frame.call(this.document.defaultView, apply);
        else
            apply();
    }
}
