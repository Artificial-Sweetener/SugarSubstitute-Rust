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
 * Present SugarCubes and SugarSubstitute settings as the Python SugarSubstitute's pages
 * (`SETTINGS_PAGES`) inside ComfyUI's settings dialog, which keeps owning values and search.
 *
 * In a plain ComfyUI browser the dialog stays ComfyUI's: only the SugarCubes page changes, with its
 * own sub-sidebar of pages that shows one page's sections at a time. In the SugarSubstitute frame
 * the pages lead the dialog's navigation as first-class entries, ComfyUI's own categories follow,
 * and other extensions' categories are offered together under "ComfyUI extensions". ComfyUI's own
 * navigation entries are never moved: entries offered elsewhere are hidden and stood in for.
 */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { createFluentIcon } from '../core/FluentIcons.js';
import { SETTINGS_PAGES, settingsPageOfSection } from './SugarCubesSettingsSections.js';
const OUR_CATEGORY = 'root/SugarCubes';
const PAGE_ATTRIBUTE = 'data-sc-settings-page';
const SHOWN_ATTRIBUTE = 'data-sc-settings-page-shown';
const HIDDEN_ATTRIBUTE = 'data-sc-settings-nav-hidden';
/** The navigation's rules: one page's sections at a time, and stand-in entries. */
export const settingsPageNavigationCss = `
  [${SHOWN_ATTRIBUTE}] .setting-group[${PAGE_ATTRIBUTE}]:not([data-sc-settings-page-current]) { display:none; }
  [${HIDDEN_ATTRIBUTE}] { display:none !important; }
  .sc-settings-pages { display:flex; flex-wrap:wrap; gap:4px; margin:0 0 16px; padding:4px;
    border-radius:8px; background:color-mix(in srgb,currentColor 4%,transparent); }
  .sc-settings-pages button, .sc-settings-nav-group [role="button"] { display:flex; align-items:center; gap:8px;
    border:0; border-radius:6px; background:transparent; color:inherit; font:inherit; cursor:pointer; }
  .sc-settings-pages button { padding:6px 10px; }
  .sc-settings-pages button[aria-current="page"], .sc-settings-nav-group [aria-current="page"] {
    background:color-mix(in srgb,currentColor 9%,transparent); font-weight:600; }
  .sc-settings-pages button:hover, .sc-settings-nav-group [role="button"]:hover {
    background:color-mix(in srgb,currentColor 6%,transparent); }
  .sc-settings-nav-group { display:flex; flex-direction:column; gap:4px; }
  .sc-settings-nav-group > h3 { margin:8px 16px 4px; font-size:12px; font-weight:600; opacity:.7; }
  .sc-settings-nav-group [role="button"] { padding:12px 16px; font-size:14px; }`;
/** One open settings dialog's page navigation. */
export class SettingsPageNavigation {
    root;
    #page = SETTINGS_PAGES[0]?.id ?? '';
    #frame = false;
    #observer;
    #inserted = [];
    constructor(root) {
        this.root = root;
        this.#observer = new HostSurfaceObserver(root, '.setting-group, [data-nav-id]', this.#apply, {
            content: true,
        });
    }
    /** Present for the frame (`true`) or for a plain ComfyUI page (`false`). */
    update(frame) {
        this.#frame = frame;
        this.#apply();
    }
    /** Remove everything this navigation added. */
    dispose() {
        this.#observer.disconnect();
        for (const element of this.#inserted.splice(0))
            element.remove();
        for (const element of this.root.querySelectorAll(`[${HIDDEN_ATTRIBUTE}],[${PAGE_ATTRIBUTE}],[${SHOWN_ATTRIBUTE}]`)) {
            element.removeAttribute(HIDDEN_ATTRIBUTE);
            element.removeAttribute(PAGE_ATTRIBUTE);
            element.removeAttribute(SHOWN_ATTRIBUTE);
            element.removeAttribute('data-sc-settings-page-current');
        }
    }
    #apply = () => {
        const groups = this.#ourGroups();
        const searching = this.#searching();
        const present = SETTINGS_PAGES.filter((page) => groups.some((group) => group.getAttribute(PAGE_ATTRIBUTE) === page.id));
        if (present.length && !present.some((page) => page.id === this.#page))
            this.#page = present[0]?.id ?? this.#page;
        const content = groups[0]?.parentElement ?? null;
        for (const group of groups)
            group.toggleAttribute('data-sc-settings-page-current', group.getAttribute(PAGE_ATTRIBUTE) === this.#page);
        content?.toggleAttribute(SHOWN_ATTRIBUTE, !searching && groups.length > 0);
        this.#subSidebar(content, present, searching);
        this.#dialogNavigation(groups.length > 0);
    };
    /** Mark SugarCubes' section groups with their page. */
    #ourGroups() {
        const groups = [];
        for (const group of this.root.querySelectorAll('.setting-group')) {
            const heading = group.querySelector('h2,h3,h4')?.textContent?.trim() ?? '';
            const page = settingsPageOfSection(heading);
            const ours = page && group.querySelector('[data-setting-id^="SugarCubes."]');
            if (!page || !ours)
                continue;
            if (group.getAttribute(PAGE_ATTRIBUTE) !== page.id)
                group.setAttribute(PAGE_ATTRIBUTE, page.id);
            groups.push(group);
        }
        return groups;
    }
    /** ComfyUI's settings search shows results across pages, unfiltered. */
    #searching() {
        const input = this.root.querySelector('input[placeholder*="Search"]');
        return (input?.value ?? '').trim() !== '';
    }
    /** In a plain ComfyUI page, the SugarCubes page's own sub-sidebar of pages. */
    #subSidebar(content, present, searching) {
        let bar = this.root.querySelector('.sc-settings-pages');
        if (this.#frame || !content || searching || present.length === 0) {
            bar?.remove();
            return;
        }
        if (!bar) {
            bar = this.root.ownerDocument.createElement('nav');
            bar.className = 'sc-settings-pages';
            bar.setAttribute('aria-label', 'SugarCubes settings pages');
            this.#inserted.push(bar);
        }
        if (bar.parentElement !== content || content.firstElementChild !== bar)
            content.prepend(bar);
        const ids = present.map((page) => page.id).join(' ');
        if (bar.dataset.pages !== ids) {
            bar.dataset.pages = ids;
            bar.replaceChildren(...present.map((page) => {
                const button = this.root.ownerDocument.createElement('button');
                button.type = 'button';
                button.dataset.page = page.id;
                button.title = page.subtitle;
                button.append(createFluentIcon(this.root.ownerDocument, page.icon, 16), page.title);
                button.addEventListener('click', () => this.#choose(page.id));
                return button;
            }));
        }
        for (const button of bar.querySelectorAll('button'))
            button.toggleAttribute('aria-current', button.dataset.page === this.#page);
        for (const button of bar.querySelectorAll('button[aria-current]'))
            button.setAttribute('aria-current', 'page');
    }
    /** In the frame: the pages lead the dialog's navigation, other extensions are grouped after. */
    #dialogNavigation(onOurCategory) {
        const ours = this.root.querySelector(`[data-nav-id="${OUR_CATEGORY}"]`);
        const nav = ours?.closest('nav');
        let pages = this.root.querySelector('.sc-settings-nav-group[data-kind="pages"]');
        let extensions = this.root.querySelector('.sc-settings-nav-group[data-kind="extensions"]');
        if (!this.#frame || !ours || !nav) {
            pages?.remove();
            extensions?.remove();
            for (const element of this.root.querySelectorAll(`[${HIDDEN_ATTRIBUTE}]`))
                element.removeAttribute(HIDDEN_ATTRIBUTE);
            return;
        }
        const document = this.root.ownerDocument;
        if (!pages) {
            pages = this.#group('pages', 'SugarSubstitute');
            for (const page of SETTINGS_PAGES) {
                const item = this.#item(page.title, createFluentIcon(document, page.icon, 16), () => {
                    this.#page = page.id;
                    if (!this.#ourGroups().length)
                        ours.click();
                    this.#apply();
                });
                item.dataset.page = page.id;
                item.title = page.subtitle;
                pages.append(item);
            }
        }
        if (nav.firstElementChild !== pages)
            nav.prepend(pages);
        for (const item of pages.querySelectorAll('[data-page]')) {
            if (onOurCategory && item.dataset.page === this.#page)
                item.setAttribute('aria-current', 'page');
            else
                item.removeAttribute('aria-current');
        }
        ours.toggleAttribute(HIDDEN_ATTRIBUTE, true);
        // Other extensions' categories are ComfyUI's plug-marked entries.
        const others = [...nav.querySelectorAll('[data-nav-id^="root/"]')].filter((item) => item !== ours &&
            !item.closest('.sc-settings-nav-group') &&
            item.querySelector('[class*="lucide--plug"]'));
        if (!extensions)
            extensions = this.#group('extensions', 'ComfyUI extensions');
        const listed = extensions.dataset.listed ?? '';
        const ids = others.map((item) => item.dataset.navId ?? '').join('|');
        if (listed !== ids) {
            extensions.dataset.listed = ids;
            extensions.replaceChildren(extensions.firstElementChild ?? document.createElement('h3'));
            for (const original of others) {
                const label = original.textContent?.trim() ?? '';
                const icon = original.querySelector('i')?.cloneNode(true);
                const item = this.#item(label, icon instanceof HTMLElement ? icon : null, () => original.click());
                extensions.append(item);
            }
        }
        for (const original of others)
            original.toggleAttribute(HIDDEN_ATTRIBUTE, true);
        const application = ours.closest('.flex.flex-col.gap-2') ?? ours.parentElement;
        if (others.length === 0)
            extensions.remove();
        else if (application && extensions.previousElementSibling !== application)
            application.after(extensions);
    }
    #group(kind, title) {
        const group = this.root.ownerDocument.createElement('div');
        group.className = 'sc-settings-nav-group';
        group.dataset.kind = kind;
        const heading = this.root.ownerDocument.createElement('h3');
        heading.textContent = title;
        group.append(heading);
        this.#inserted.push(group);
        return group;
    }
    #item(label, icon, select) {
        const item = this.root.ownerDocument.createElement('div');
        item.setAttribute('role', 'button');
        item.tabIndex = 0;
        if (icon)
            item.append(icon);
        const text = this.root.ownerDocument.createElement('span');
        text.textContent = label;
        item.append(text);
        item.addEventListener('click', select);
        item.addEventListener('keydown', (event) => {
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                select();
            }
        });
        return item;
    }
    #choose(page) {
        this.#page = page;
        this.#apply();
    }
}
