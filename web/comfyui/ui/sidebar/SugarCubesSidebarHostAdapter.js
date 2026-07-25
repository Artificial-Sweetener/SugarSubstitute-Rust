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
/** Adapt the SugarCubes browser into Comfy's custom sidebar contract. */
/** Own sidebar registration, mount reuse, and DOM composition. */
export class SugarCubesSidebarHostAdapter {
    #document;
    #getManager;
    #browser;
    #logger;
    #registeredManager = null;
    #root = null;
    /** Bind the host manager and embedded Cube browser. */
    constructor(options) {
        this.#document = options.document;
        this.#getManager = options.getManager;
        this.#browser = options.browser;
        this.#logger = options.logger;
    }
    /** Register once for each host extension-manager identity. */
    register() {
        const manager = this.#getManager();
        if (!manager?.registerSidebarTab) {
            this.#logger.warn('SugarCubes: extension manager unavailable; sidebar tab not registered.');
            return;
        }
        if (this.#registeredManager === manager)
            return;
        this.#root = null;
        manager.registerSidebarTab({
            id: 'sugarcubes',
            title: 'SugarCubes',
            tooltip: 'SugarCubes',
            icon: 'mdi mdi-cube',
            type: 'custom',
            render: (container) => this.#render(container),
            destroy: () => this.#root?.remove(),
        });
        this.#registeredManager = manager;
    }
    /** Render the lazily composed browser panel into Comfy's sidebar host. */
    #render(container) {
        this.#root ??= this.#buildRoot();
        container.replaceChildren(this.#root);
    }
    /** Build the extension-owned sidebar DOM without dynamic HTML parsing. */
    #buildRoot() {
        const root = this.#document.createElement('div');
        root.className = 'sugarcubes-sidebar-panel';
        Object.assign(root.style, {
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
            padding: '12px',
            color: 'var(--fg-color, #e8e8e8)',
            fontFamily: 'sans-serif',
        });
        const header = this.#document.createElement('div');
        header.textContent = 'SugarCubes';
        Object.assign(header.style, {
            fontSize: '14px',
            fontWeight: '600',
            letterSpacing: '0.02em',
        });
        const content = this.#document.createElement('div');
        content.className = 'sugarcubes-sidebar-panel__content';
        Object.assign(content.style, {
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
        });
        const library = this.#document.createElement('div');
        library.className = 'sugarcubes-sidebar-panel__library';
        Object.assign(library.style, {
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
        });
        const browser = this.#document.createElement('div');
        browser.className = 'sugarcubes-sidebar-panel__browser';
        Object.assign(browser.style, {
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
        });
        library.append(browser);
        content.append(library);
        root.append(header, content);
        this.#browser.mountEmbedded(browser);
        return root;
    }
}
