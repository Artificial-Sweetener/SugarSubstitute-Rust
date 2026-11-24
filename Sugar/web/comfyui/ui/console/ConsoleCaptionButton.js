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
 * The console's button in the SugarSubstitute frame's title-bar row, beside the window buttons.
 *
 * It looks and behaves like a window caption button (the shared caption button record): flat,
 * with hover and press fills only. Open or closed shows through its icon alone, which shows where
 * the button goes: the solid bar while the console is closed, the hollow bar while it is open.
 * Outside the frame there is no title-bar row, so there is no console button.
 */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { captionButtonCss, sharedIconUrl } from '../core/SharedElements.js';
const FRAME = '.sc-substitute-frame';
const STYLE_ID = 'sugarcubes-console-caption-styles';
/**
 * The frame marks itself `open` or `closed` while the button sits in its row, so its own controls
 * make room and its menu shows the console entry's destination.
 */
export const CONSOLE_CAPTION_ATTRIBUTE = 'data-console-caption';
/** Install the caption button stylesheet once per document. */
function ensureStyles(document) {
    if (document.getElementById(STYLE_ID))
        return;
    const style = document.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
    .sc-console-caption { position:absolute; top:0; right:var(--sc-native-caption-width,0px); z-index:1; }
    ${captionButtonCss('.sc-console-caption')}`;
    document.head.append(style);
}
/** One console button, kept in the frame's title-bar row as the frame mounts and remounts. */
export class ConsoleCaptionButton {
    document;
    #button;
    #icon;
    #observer = null;
    #disposed = false;
    /** Build the button; it shows nothing until the frame mounts. */
    constructor(document, toggle) {
        this.document = document;
        ensureStyles(document);
        this.#button = document.createElement('button');
        this.#button.type = 'button';
        this.#button.className = 'sc-console-caption';
        this.#button.setAttribute('aria-label', 'Console');
        this.#icon = document.createElement('span');
        this.#icon.setAttribute('aria-hidden', 'true');
        this.#button.append(this.#icon);
        this.#button.addEventListener('click', toggle);
        this.update(false);
    }
    /** Follow frame mounts and place the button. */
    setup() {
        if (this.#observer || this.#disposed)
            return;
        this.#observer = new HostSurfaceObserver(this.document.body, FRAME, this.#attach);
        this.#attach();
    }
    /** Show where the button goes from the console's state; aria and the tooltip say the state. */
    update(open) {
        this.#button.setAttribute('aria-pressed', String(open));
        this.#button.title = open ? 'Hide console' : 'Show console';
        this.#button.dataset.open = String(open);
        if (this.#button.parentElement)
            this.#button.parentElement.setAttribute(CONSOLE_CAPTION_ATTRIBUTE, open ? 'open' : 'closed');
        this.#icon.style.setProperty('--sc-caption-icon', `url("${sharedIconUrl(open ? 'console-close' : 'console-open')}")`);
    }
    /** Remove the button and stop following the frame. */
    dispose() {
        this.#disposed = true;
        this.#observer?.disconnect();
        this.#button.parentElement?.removeAttribute(CONSOLE_CAPTION_ATTRIBUTE);
        this.#button.remove();
    }
    /** Sit in the frame's row, which exists only while the SugarSubstitute frame is shown. */
    #attach = () => {
        if (this.#disposed)
            return;
        const frame = this.document.querySelector(FRAME);
        if (!frame) {
            this.#button.parentElement?.removeAttribute(CONSOLE_CAPTION_ATTRIBUTE);
            this.#button.remove();
            return;
        }
        if (this.#button.parentElement !== frame) {
            this.#button.parentElement?.removeAttribute(CONSOLE_CAPTION_ATTRIBUTE);
            frame.append(this.#button);
        }
        frame.setAttribute(CONSOLE_CAPTION_ATTRIBUTE, this.#button.dataset.open === 'true' ? 'open' : 'closed');
    };
}
