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
/** Preview on the tab bar the workflow tab that a file dragged from the desktop would open. */
import type { DraggedFiles } from '../../core/file_drag/DraggedFiles.js';
import type { WorkflowTabDropTarget } from './WorkflowTabGhost.js';

/** What the preview needs from the frame. */
export interface FileTabDropHost {
  readonly document: Document;
  /** The names of the dragged files, which only a host that can see the drag supplies. */
  readonly dragged: DraggedFiles;
  readonly tabBar: Pick<WorkflowTabDropTarget, 'hit' | 'preview'>;
  /** Put the next workflow that opens at this tab position; null withdraws the request. */
  placeNextOpened(index: number | null): void;
}

/** File types ComfyUI opens as a workflow of their own. */
const WORKFLOW_FILE = /\.(json|png|webp)$/i;

/** The title ComfyUI gives the tab of a dropped file, or null when the file opens no workflow. */
export function fileTabTitle(name: string): string | null {
  if (!WORKFLOW_FILE.test(name)) return null;
  return name.replace(/\.\w+$/, '') || null;
}

/**
 * While a file hangs over the tab bar, show the dashed tab it would open as; when it is dropped
 * there, have the workflow ComfyUI opens take that place.
 *
 * ComfyUI itself opens the dropped file. A page cannot read a dragged file's name before the drop,
 * so without a host that names the drag there is nothing to preview and the drop opens the workflow
 * where ComfyUI puts it.
 */
export class WorkflowFileTabDrop {
  #pointer: { readonly x: number; readonly y: number } | null = null;
  #names: readonly string[] | null = null;
  #shown: number | null = null;
  #stop: (() => void) | null = null;
  #awaitingOpen = false;

  constructor(private readonly host: FileTabDropHost) {}

  /** Start following file drags over the document. */
  setup(): void {
    if (this.#stop) return;
    const { document, dragged } = this.host;
    const unsubscribe = dragged.subscribe(this.#named);
    document.addEventListener('dragover', this.#over, true);
    document.addEventListener('dragleave', this.#leave, true);
    document.addEventListener('drop', this.#drop, true);
    this.#stop = () => {
      unsubscribe();
      document.removeEventListener('dragover', this.#over, true);
      document.removeEventListener('dragleave', this.#leave, true);
      document.removeEventListener('drop', this.#drop, true);
    };
  }

  /** Stop following drags and withdraw anything shown or requested. */
  dispose(): void {
    this.#stop?.();
    this.#stop = null;
    this.#pointer = null;
    this.#names = null;
    this.#render();
    this.#forget();
  }

  /**
   * The host named a drag or said it ended. Its word that a drag ended can arrive before the
   * page's own drop, so the names are kept until the page sees the drag end too.
   */
  readonly #named = (): void => {
    const names = this.host.dragged.names;
    if (names || !this.#pointer) this.#names = names;
    this.#render();
  };

  readonly #over = (event: DragEvent): void => {
    if (![...(event.dataTransfer?.types ?? [])].includes('Files')) return;
    this.#forget();
    this.#pointer = { x: event.clientX, y: event.clientY };
    this.#render();
  };

  /** A drag leaves the window when it leaves an element for nothing. */
  readonly #leave = (event: DragEvent): void => {
    if (event.relatedTarget !== null) return;
    this.#ended();
  };

  readonly #drop = (): void => {
    const index = this.#shown;
    this.#ended();
    if (index === null) return;
    this.host.placeNextOpened(index);
    this.#awaitingOpen = true;
    // The file may open no workflow; the request ends when the person does anything else.
    this.host.document.addEventListener('pointerdown', this.#forget, true);
    this.host.document.addEventListener('keydown', this.#forget, true);
  };

  #ended(): void {
    this.#pointer = null;
    this.#names = this.host.dragged.names;
    this.#render();
  }

  /** Withdraw a placement request that no opened workflow took. */
  readonly #forget = (): void => {
    if (!this.#awaitingOpen) return;
    this.#awaitingOpen = false;
    this.host.placeNextOpened(null);
    this.host.document.removeEventListener('pointerdown', this.#forget, true);
    this.host.document.removeEventListener('keydown', this.#forget, true);
  };

  #render(): void {
    const name = this.#names?.[0];
    const title = name === undefined ? null : fileTabTitle(name);
    const index =
      title !== null && this.#pointer
        ? this.host.tabBar.hit(this.#pointer.x, this.#pointer.y)
        : null;
    if (index === null && this.#shown === null) return;
    this.#shown = index;
    this.host.tabBar.preview(index === null ? null : title, index ?? 0);
  }
}
