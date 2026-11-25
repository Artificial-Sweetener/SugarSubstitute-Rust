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
 * In the SugarSubstitute frame, ComfyUI's sidebar strip is hidden, so the panel a tab opens on
 * the left gets its own close button. Job history is the exception: the frame shows it as its
 * own generation queue, which closes itself.
 */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { createFluentIcon } from '../../core/FluentIcons.js';
import { GENERATION_QUEUE_DOCK } from './GenerationQueueDock.js';

const PANEL = '.side-bar-panel';
const STYLE_ID = 'sugarcubes-sidebar-close-styles';

/** Keep one close button on the open sidebar panel while the frame is shown. */
export class SidebarPanelClose {
  readonly #button: HTMLButtonElement;
  #observer: HostSurfaceObserver | null = null;

  constructor(
    private readonly document: Document,
    close: () => void,
  ) {
    if (!document.getElementById(STYLE_ID)) {
      const style = document.createElement('style');
      style.id = STYLE_ID;
      style.textContent = `
        :root[data-sc-substitute-frame] ${PANEL} { position:relative; }
        .sc-sidebar-close { position:absolute; top:6px; right:6px; z-index:5; display:inline-flex;
          align-items:center; justify-content:center; width:28px; height:28px; padding:0; border:0;
          border-radius:4px; background:transparent; color:var(--sc-text,var(--fg-color,#fff)); cursor:pointer; }
        .sc-sidebar-close:hover { background:color-mix(in srgb,currentColor 9%,transparent); }
        .sc-sidebar-close:active { background:color-mix(in srgb,currentColor 6%,transparent); }
        .sc-sidebar-close:focus-visible { outline:2px solid var(--sc-accent-color,currentColor); outline-offset:-2px; }`;
      document.head.append(style);
    }
    this.#button = document.createElement('button');
    this.#button.type = 'button';
    this.#button.className = 'sc-sidebar-close';
    this.#button.setAttribute('aria-label', 'Close panel');
    this.#button.title = 'Close panel';
    this.#button.append(createFluentIcon(document, 'dismiss', 16));
    this.#button.addEventListener('click', close);
  }

  /** Follow panel mounts and content changes. */
  setup(): void {
    if (this.#observer) return;
    this.#observer = new HostSurfaceObserver(this.document.body, PANEL, this.#attach, {
      content: true,
    });
    this.#attach();
  }

  /** Remove the button with the frame. */
  dispose(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    this.#button.remove();
  }

  readonly #attach = (): void => {
    const panel = this.document.querySelector<HTMLElement>(PANEL);
    if (!panel || panel.querySelector(GENERATION_QUEUE_DOCK)) {
      this.#button.remove();
      return;
    }
    if (this.#button.parentElement !== panel) panel.append(this.#button);
  };
}
