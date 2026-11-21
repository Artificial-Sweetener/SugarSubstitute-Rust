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
 * Own the console: where it opens, whether it is open, how tall it is, and its feed.
 *
 * Inside the SugarSubstitute frame the console is SugarCubes' own panel, styled as the Python
 * application's Comfy Console and docked under the card column. Outside the frame ComfyUI's own
 * Logs panel is the console, so the same command opens that. Visibility and height are a viewer's
 * convenience, kept in this browser's storage.
 */
import { isRecord } from '../types/common.js';
import { ComfyLogFeed } from './ComfyLogFeed.js';
import { CONSOLE_DEFAULT_HEIGHT, CONSOLE_MINIMUM_HEIGHT, ConsolePanel } from './ConsolePanel.js';
import { ConsoleToolbarButton } from './ConsoleToolbarButton.js';
/** ComfyUI's command that shows or hides its Logs bottom panel. */
export const COMFY_LOGS_COMMAND = 'Workspace.ToggleBottomPanelTab.logs-terminal';
const STORAGE_KEY = 'sugarcubes.console';
/** Whether `event` is ComfyUI's console key, Ctrl+` with no other modifier. */
function isConsoleKey(event) {
    return event.key === '`' && event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey;
}
/** One console for the page. */
export class WorkspaceConsole {
    #options;
    #button;
    #panel = null;
    #feed = null;
    #host = null;
    #visible = false;
    #height = CONSOLE_DEFAULT_HEIGHT;
    /** Bind to the page; nothing is shown until `setup`. */
    constructor(options) {
        this.#options = options;
        this.#button = new ConsoleToolbarButton(options.document, () => this.toggle());
        const saved = this.#read();
        this.#visible = saved.visible;
        this.#height = saved.height;
    }
    /** Place the toolbar button, take ComfyUI's console key, and reopen a console left open. */
    setup() {
        this.#button.setup();
        // Capture on the window runs before ComfyUI's keybinding handler, which listens there too.
        this.#options.document.defaultView?.addEventListener('keydown', this.#onKey, true);
        this.#button.update(this.#visible && this.#substitute());
        if (this.#visible && this.#substitute())
            this.#show();
    }
    /** Whether the frame's own console is open. */
    get visible() {
        return this.#visible && this.#panel !== null;
    }
    /** Show or hide the console: the frame's own inside SugarSubstitute, ComfyUI's Logs elsewhere. */
    toggle() {
        if (!this.#substitute()) {
            this.#runComfyCommand(COMFY_LOGS_COMMAND);
            return;
        }
        this.#visible = !this.visible;
        if (this.#visible)
            this.#show();
        else
            this.#hide();
        this.#button.update(this.#visible);
        this.#write();
    }
    /** Release the panel, its feed and the button with the page. */
    dispose() {
        this.#options.document.defaultView?.removeEventListener('keydown', this.#onKey, true);
        this.#hide();
        this.#button.dispose();
    }
    /**
     * Inside the frame, ComfyUI's console key (Ctrl+`) opens the frame's console instead of
     * ComfyUI's Logs panel. ComfyUI owns that binding, so SugarCubes declares none of its own:
     * elsewhere the key reaches ComfyUI untouched and opens its Logs as it always has.
     */
    #onKey = (event) => {
        if (!this.#substitute() || !isConsoleKey(event))
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        if (!event.repeat)
            this.toggle();
    };
    /** Whether the SugarSubstitute frame presents this page. */
    #substitute() {
        return this.#options.document.documentElement.hasAttribute('data-sc-substitute-frame');
    }
    /** Dock the panel into the card column and start following ComfyUI's output. */
    #show() {
        const host = this.#options.getHost();
        if (!host)
            return;
        if (!this.#panel) {
            this.#panel = new ConsolePanel(this.#options.document, (text) => this.#copy(text), (height) => this.#resize(height));
        }
        this.#host = host;
        host.append(this.#panel.element);
        this.#applyHeight();
        this.#connect();
    }
    /** Close the panel, end the feed and give the card column its full height back. */
    #hide() {
        const feed = this.#feed;
        this.#feed = null;
        if (feed)
            void feed.close().catch(this.#options.report);
        this.#panel?.dispose();
        this.#panel = null;
        this.#host?.style.removeProperty('--sc-console-height');
        this.#host?.style.removeProperty('--sc-console-inset');
        this.#host = null;
    }
    /** Replay ComfyUI's kept output and follow it while the panel is open. */
    #connect() {
        if (this.#feed)
            return;
        const api = isRecord(this.#options.app) ? this.#options.app.api : null;
        let feed;
        try {
            feed = new ComfyLogFeed(api);
        }
        catch (error) {
            this.#options.report(error);
            return;
        }
        this.#feed = feed;
        const panel = this.#panel;
        feed
            .open((lines) => this.#panel?.append(lines))
            .then((kept) => {
            if (this.#feed === feed && panel === this.#panel)
                panel?.append(kept);
        })
            .catch(this.#options.report);
    }
    /** Keep a new height from the resize handle. */
    #resize(height) {
        this.#height = height;
        this.#applyHeight();
        this.#write();
    }
    /** Publish the panel's height to the column, which the masonry above it leaves free. */
    #applyHeight() {
        const height = `${String(Math.max(CONSOLE_MINIMUM_HEIGHT, this.#height))}px`;
        this.#host?.style.setProperty('--sc-console-height', height);
        this.#host?.style.setProperty('--sc-console-inset', height);
    }
    /** Hand the shown output to the clipboard. */
    #copy(text) {
        const clipboard = this.#options.document.defaultView?.navigator.clipboard;
        if (!clipboard)
            return;
        void clipboard.writeText(text).catch(this.#options.report);
    }
    /** Run one of ComfyUI's own commands through its command service. */
    #runComfyCommand(id) {
        const app = isRecord(this.#options.app) ? this.#options.app : {};
        const manager = isRecord(app.extensionManager) ? app.extensionManager : {};
        const command = isRecord(manager.command) ? manager.command : {};
        const execute = command.execute;
        if (typeof execute !== 'function') {
            this.#options.report(new Error(`ComfyUI cannot run ${id}.`));
            return;
        }
        void Promise.resolve(execute.call(command, id)).catch(this.#options.report);
    }
    /** The saved visibility and height, or the defaults when storage is unavailable. */
    #read() {
        try {
            const raw = this.#options.document.defaultView?.localStorage.getItem(STORAGE_KEY);
            const value = raw ? JSON.parse(raw) : null;
            if (!isRecord(value))
                return { visible: false, height: CONSOLE_DEFAULT_HEIGHT };
            return {
                visible: value.visible === true,
                height: typeof value.height === 'number' && Number.isFinite(value.height)
                    ? value.height
                    : CONSOLE_DEFAULT_HEIGHT,
            };
        }
        catch {
            return { visible: false, height: CONSOLE_DEFAULT_HEIGHT };
        }
    }
    /** Remember visibility and height for this browser, where storage allows. */
    #write() {
        try {
            this.#options.document.defaultView?.localStorage.setItem(STORAGE_KEY, JSON.stringify({ visible: this.#visible, height: this.#height }));
        }
        catch {
            // Storage refused (private mode or blocked): the console still works for this page.
        }
    }
}
