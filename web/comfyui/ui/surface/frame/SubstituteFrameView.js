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
import { createButton } from '../appearance/ControlFactory.js';
import { WorkflowTabsView } from './WorkflowTabsView.js';
import { AppOrbView } from './AppOrbView.js';
import { WorkflowTabStripLayout } from './WorkflowTabStripLayout.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
import { applyOwnedTheme } from '../appearance/ThemeService.js';
import { createCubeAddIconElement } from '../CubeAddIcon.js';
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
    #stackGlyph;
    #addGlyph;
    #stopStack;
    #presented = [];
    #dark = null;
    #initialized = false;
    /** Create provider controls with stable child lifetimes and shared frame artwork. */
    constructor(document, actions) {
        this.document = document;
        this.actions = actions;
        this.element = document.createElement('header');
        this.element.className = 'sc-substitute-frame';
        this.element.setAttribute('data-sc-style-theme-root', '');
        this.#menu = new ContextMenu(document);
        this.#overflowMenu = new ContextMenu(document);
        const viewport = document.createElement('div');
        viewport.className = 'sc-workflow-strip';
        viewport.id = 'sc-workflow-strip';
        viewport.setAttribute('data-sc-scrollbar-managed', '');
        this.#tabs = new WorkflowTabsView(document, actions.workflows, viewport);
        this.#orb = new AppOrbView(document, () => [
            { label: 'Open workflow', select: () => actions.command('Comfy.OpenWorkflow') },
            { label: 'Save', select: () => actions.command('Comfy.SaveWorkflow') },
            { label: 'Save as', select: () => actions.command('Comfy.SaveWorkflowAs') },
            { label: 'Export workflow', select: () => actions.command('Comfy.ExportWorkflow') },
            { label: '', separator: true },
            { label: 'Settings', select: () => actions.command('Comfy.ShowSettingsDialog') },
        ], actions.report);
        const create = this.#button('New workflow', '', () => actions.workflows.create());
        create.classList.add('sc-workflow-new');
        const plus = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        plus.classList.add('sc-workflow-new-glyph');
        plus.setAttribute('viewBox', '0 0 12 12');
        plus.setAttribute('aria-hidden', 'true');
        const cross = document.createElementNS('http://www.w3.org/2000/svg', 'path');
        cross.setAttribute('d', 'M0 6H12M6 0V12');
        cross.setAttribute('stroke', 'currentColor');
        cross.setAttribute('stroke-width', '1');
        cross.setAttribute('fill', 'none');
        plus.append(cross);
        create.append(plus);
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
        const previous = this.#button('Scroll workflows left', '', () => undefined);
        const next = this.#button('Scroll workflows right', '', () => undefined);
        previous.classList.add('sc-workflow-scroll', 'sc-workflow-scroll--previous');
        next.classList.add('sc-workflow-scroll');
        viewport.append(this.#tabs.element);
        this.#stripLayout = new WorkflowTabStripLayout(document, this.element, viewport, this.#tabs.element, { overflow, previous, next }, (width) => this.#tabs.resize(width), () => this.#overflowMenu.close(), () => this.#tabs.refreshScrollLayout());
        const cluster = document.createElement('div');
        cluster.className = 'sc-orb-actions';
        this.#stackButton = this.#button('Cube stack', '', () => {
            if (actions.stack.available)
                void actions.stack.activate().catch(actions.report);
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
        this.#stackGlyph = this.#glyph('sc-orb-stack-icon', 'panel-left.svg');
        this.#addGlyph = createCubeAddIconElement(document);
        // Fit the shared 16px cube and its 3px badge overhang to the replaced 13px stack glyph.
        Object.assign(this.#addGlyph.style, {
            position: 'absolute',
            left: '5px',
            top: '18.5px',
            transform: `scale(${String(13 / 19)})`,
            transformOrigin: 'top left',
        });
        this.#stackButton.append(this.#stackGlyph, this.#addGlyph);
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
        this.element.append(this.#orb.element, previous, viewport, next, create, overflow, cluster);
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
            applyOwnedTheme(this.element, dark);
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
    /** Present the stack owner's current cart or compact intent with the shared cube-chrome icon. */
    #updateStack() {
        const action = this.actions.stack.action;
        const compact = action === 'expand';
        const label = action === 'add' ? 'Add Cube' : compact ? 'Expand Cube stack' : 'Collapse Cube stack';
        this.#stackButton.setAttribute('aria-label', label);
        this.#stackButton.toggleAttribute('disabled', !this.actions.stack.available);
        this.#stackButton.title = this.actions.stack.available
            ? label
            : 'Switch to masonry to use Cube stack controls';
        if (action === 'add') {
            this.#stackButton.removeAttribute('aria-pressed');
            this.#stackButton.setAttribute('aria-haspopup', 'dialog');
        }
        else {
            this.#stackButton.setAttribute('aria-pressed', String(compact));
            this.#stackButton.removeAttribute('aria-haspopup');
        }
        this.#addGlyph.style.display = action === 'add' ? '' : 'none';
        this.#stackGlyph.style.display = action === 'add' ? 'none' : '';
        const filename = compact ? 'panel-left-regular.svg' : 'panel-left.svg';
        this.#stackGlyph.style.setProperty('--sc-orb-glyph', `url("${new URL(`../../../../orb/${filename}`, import.meta.url).href}")`);
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
        const button = createButton(this.document);
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
