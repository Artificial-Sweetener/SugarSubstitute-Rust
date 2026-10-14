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
/** Compose the source-matched frame controls independently of host mounts and workflow stores. */
import { WorkflowTabsView } from './WorkflowTabsView.js';
import { AppOrbView } from './AppOrbView.js';
import { WorkflowTabStripLayout } from './WorkflowTabStripLayout.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
import { applyOwnedFluentTheme } from '../appearance/FluentThemeRuntime.js';
/** Own frame rendering and interaction while collaborators retain all application policies. */
export class SubstituteFrameView {
    document;
    actions;
    element;
    #tabs;
    #orb;
    #menu;
    #overflowMenu;
    #stripLayout;
    #stackButton;
    #stopStack;
    #presented = [];
    #dark = null;
    #initialized = false;
    /** Create official Fluent controls with stable child lifetimes and exact reference artwork. */
    constructor(document, actions) {
        this.document = document;
        this.actions = actions;
        this.element = document.createElement('header');
        this.element.className = 'sc-substitute-frame';
        this.element.setAttribute('data-sc-fluent-theme-root', '');
        this.#menu = new ContextMenu(document);
        this.#overflowMenu = new ContextMenu(document);
        const viewport = document.createElement('div');
        viewport.className = 'sc-workflow-strip';
        this.#tabs = new WorkflowTabsView(document, actions.workflows, viewport);
        this.#orb = new AppOrbView(document, () => [
            { label: 'Open workflow', select: () => actions.command('Comfy.OpenWorkflow') },
            { label: 'Save', select: () => actions.command('Comfy.SaveWorkflow') },
            { label: 'Save as', select: () => actions.command('Comfy.SaveWorkflowAs') },
            { label: 'Export workflow', select: () => actions.command('Comfy.ExportWorkflow') },
            { label: '', separator: true },
            { label: 'Settings', select: () => actions.command('Comfy.ShowSettingsDialog') },
        ]);
        const create = this.#button('New workflow', '+', () => actions.workflows.create());
        create.classList.add('sc-workflow-new');
        const overflow = this.#button('More workflows', '⌄', () => this.#overflowMenu.open({
            label: 'Open workflows',
            trigger: overflow,
            anchor: { kind: 'below', bounds: overflow.getBoundingClientRect() },
            entries: this.#presented.map((tab) => ({
                label: tab.title,
                checked: tab.selected,
                selection: 'single',
                select: () => actions.workflows.activate(tab.id),
            })),
        }));
        overflow.classList.add('sc-workflow-overflow');
        overflow.setAttribute('aria-haspopup', 'menu');
        viewport.append(this.#tabs.element, create);
        this.#stripLayout = new WorkflowTabStripLayout(document, this.element, viewport, this.#tabs.element, overflow, () => this.#tabs.resize(), () => this.#overflowMenu.close());
        const cluster = document.createElement('div');
        cluster.className = 'sc-orb-actions';
        this.#stackButton = this.#button('Cube stack', '', () => {
            void actions.stack.toggleCompact().catch(actions.report);
        });
        this.#stackButton.addEventListener('contextmenu', (event) => {
            event.preventDefault();
            this.#menu.open({
                label: 'Cube stack',
                trigger: this.#stackButton,
                anchor: { kind: 'below', bounds: this.#stackButton.getBoundingClientRect() },
                entries: actions.stack.menu(),
            });
        });
        this.#stackButton.append(this.#glyph('sc-orb-stack-icon', 'panel-left.svg'));
        this.#stopStack = actions.stack.subscribe(() => this.#updateStack());
        this.#updateStack();
        const overrides = this.#button('Set Global Override', '', () => actions.openOverrides(overrides));
        overrides.setAttribute('aria-haspopup', 'menu');
        overrides.setAttribute('aria-expanded', 'false');
        const chevron = document.createElement('span');
        chevron.className = 'sc-orb-chevron';
        chevron.setAttribute('aria-hidden', 'true');
        overrides.append(this.#glyph('sc-orb-pin-icon', 'pin.svg'), chevron);
        cluster.append(this.#stackButton, overrides);
        this.element.append(this.#orb.element, viewport, overflow, cluster);
    }
    /** Retain tab identities through workflow and activity updates. */
    update(tabs, unread) {
        this.#presented = tabs;
        this.#stripLayout.update(tabs.length);
        this.#tabs.update(tabs, unread, this.#initialized);
        this.#initialized = true;
    }
    /** Project the accepted palette only when it changes and let the orb retain raster variants. */
    setPalette(color, dark) {
        if (dark !== this.#dark) {
            this.#dark = dark;
            this.element.dataset.colorScheme = dark ? 'dark' : 'light';
            applyOwnedFluentTheme(this.element, dark);
        }
        void this.#orb.update(color, dark).catch(this.actions.report);
    }
    /** Release rendering subscriptions, Microsoft focus groups and transient menus together. */
    dispose() {
        this.#stopStack();
        this.#stripLayout.dispose();
        this.#overflowMenu.close();
        this.#menu.close();
        this.#orb.dispose();
        this.#tabs.dispose();
        this.element.remove();
    }
    /** Keep the original compact toggle and glyph synchronized with the rail's preference owner. */
    #updateStack() {
        const compact = this.actions.stack.compact;
        const label = compact ? 'Expand Cube stack' : 'Collapse Cube stack';
        this.#stackButton.setAttribute('aria-label', label);
        this.#stackButton.title = label;
        this.#stackButton.setAttribute('aria-pressed', String(compact));
        const filename = compact ? 'panel-left-regular.svg' : 'panel-left.svg';
        this.#stackButton
            .querySelector('.sc-orb-stack-icon')
            ?.style.setProperty('--sc-orb-glyph', `url("${new URL(`../../../../orb/${filename}`, import.meta.url).href}")`);
    }
    /** Load trusted packaged artwork through a theme-tinted mask. */
    #glyph(className, filename) {
        const glyph = this.document.createElement('span');
        glyph.className = className;
        glyph.setAttribute('aria-hidden', 'true');
        glyph.style.setProperty('--sc-orb-glyph', `url("${new URL(`../../../../orb/${filename}`, import.meta.url).href}")`);
        return glyph;
    }
    /** Give each official Fluent action a stable accessible name and explicit error boundary. */
    #button(label, text, action) {
        const button = this.document.createElement('fluent-button');
        button.className = 'sc-frame-tool';
        button.setAttribute('appearance', 'subtle');
        button.setAttribute('aria-label', label);
        button.title = label;
        button.textContent = text;
        button.addEventListener('click', () => {
            try {
                action();
            }
            catch (error) {
                this.actions.report(error);
            }
        });
        return button;
    }
}
