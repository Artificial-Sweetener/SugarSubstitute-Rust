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
/** Own Cube browser shell DOM construction and host event binding. */

import { $el } from '/scripts/ui.js';
import type { BrowserElements, BrowserViewHandlers } from './CubeBrowserView.js';
import type { CubeBrowserVersionView } from './CubeBrowserVersionView.js';

interface BuilderOptions {
  documentRef: Document | null;
  windowRef: Window | null;
  versions: CubeBrowserVersionView;
  getHandlers(): BrowserViewHandlers;
  isActive(): boolean;
}

/** Build the browser shell while delegated views own its dynamic regions. */
export class CubeBrowserViewBuilder {
  constructor(private readonly options: BuilderOptions) {}

  /** Construct and bind the complete browser shell. */
  build(): BrowserElements {
    const documentRef = this.options.documentRef ?? document;
    const createIcon = (...classes: string[]): HTMLElement => {
      const icon = documentRef.createElement('i');
      icon.classList.add(...classes);
      return icon;
    };
    const action = (
      className: string,
      title: string,
      outlineIcon: string,
      filledIcon: string,
    ): HTMLButtonElement =>
      $el('button', { className, type: 'button', title, disabled: true }, [
        createIcon('mdi', outlineIcon),
        createIcon('mdi', filledIcon),
      ]);
    const dialog = $el('div', { className: 'sugarcubes-browser' });
    const searchInput = $el('input.p-inputtext.p-component', {
      type: 'text',
      placeholder: 'Search cubes...',
      autocomplete: 'off',
    });
    const listBody = $el('div', { className: 'sugarcubes-browser__list-body' });
    const emptyState = $el('div.sugarcubes-browser__empty', 'No cubes found. Try exporting one.');
    const listContainer = $el('div.sugarcubes-browser__list', [
      $el('div.sugarcubes-browser__list-header', [
        $el('div.sugarcubes-browser__search', [searchInput]),
      ]),
      listBody,
      emptyState,
    ]);
    const detailTitle = $el('h3', 'Select a cube to see details');
    const detailIcon = $el('span.sugarcubes-cube-icon.is-generic');
    const detailMeta = $el('div.sugarcubes-browser__meta');
    const detailDescription = $el('pre');
    const previewCanvas = $el('canvas', { className: 'sugarcubes-browser__preview-canvas' });
    const previewStatus = $el('div.sugarcubes-browser__preview-status', '');
    const previewContainer = $el('div.sugarcubes-browser__preview', [previewCanvas, previewStatus]);
    const favoriteButton = $el('button', {
      className: 'sugarcubes-browser__favorite is-empty',
      type: 'button',
      title: 'Mark as favourite',
      textContent: '\u2606',
    });
    const editButton = action(
      'sugarcubes-browser__edit',
      'Edit cube metadata',
      'mdi-pencil-outline',
      'mdi-pencil',
    );
    const editSaveButton = action(
      'sugarcubes-browser__edit-save sugarcubes-browser__action-hidden',
      'Save changes',
      'mdi-content-save-outline',
      'mdi-content-save',
    );
    const editCancelButton = action(
      'sugarcubes-browser__edit-cancel sugarcubes-browser__action-hidden',
      'Cancel edits',
      'mdi-close-circle-outline',
      'mdi-close-circle',
    );
    const deleteButton = action(
      'sugarcubes-browser__delete',
      'Delete cube',
      'mdi-trash-can-outline',
      'mdi-trash-can',
    );
    const promoteButton = action(
      'sugarcubes-browser__promote',
      'Move to cube pack',
      'mdi-package-up',
      'mdi-package-up',
    );
    const placeButton = action(
      'sugarcubes-browser__place',
      'Place',
      'mdi-arrow-right-bold-outline',
      'mdi-arrow-right-bold',
    );
    const version = this.options.versions.build();
    const detailHeader = $el('div.sugarcubes-browser__detail-header', [
      $el('div.sugarcubes-browser__detail-title', [
        favoriteButton,
        detailIcon,
        detailTitle,
        version.control,
      ]),
      $el('div.sugarcubes-browser__detail-actions', [
        editButton,
        editSaveButton,
        promoteButton,
        deleteButton,
        editCancelButton,
        placeButton,
      ]),
    ]);
    const detailContainer = $el('div.sugarcubes-browser__detail', [
      detailHeader,
      $el('div.sugarcubes-browser__detail-body', [detailMeta, detailDescription]),
    ]);
    const detailStack = $el(
      'div',
      {
        style: {
          display: 'flex',
          flexDirection: 'column',
          gap: '12px',
          minWidth: 0,
          flex: '0 1 auto',
        },
      },
      [detailContainer, previewContainer],
    );
    dialog.append($el('div.sugarcubes-browser__content', [listContainer, detailStack]));
    const handlers = () => this.options.getHandlers();
    placeButton.addEventListener('click', () => handlers().onPlace?.());
    favoriteButton.addEventListener('click', () => handlers().onFavoriteToggle?.());
    editButton.addEventListener('click', () => handlers().onEditToggle?.());
    editSaveButton.addEventListener('click', () => handlers().onEditSave?.());
    editCancelButton.addEventListener('click', () => handlers().onEditCancel?.());
    deleteButton.addEventListener('click', () => handlers().onDelete?.());
    promoteButton.addEventListener('click', () => handlers().onPromote?.());
    searchInput.addEventListener('input', (event) => {
      handlers().onSearchChange?.(
        event.target instanceof HTMLInputElement ? event.target.value : '',
      );
    });
    listBody.addEventListener('click', (event) => {
      const target = event.target instanceof Element ? event.target : null;
      const header = target?.closest<HTMLElement>('[data-author]');
      if (header) {
        handlers().onToggleAuthorGroup?.(header.dataset.author || '');
        return;
      }
      const row = target?.closest<HTMLElement>('[data-cube]');
      if (row) handlers().onSelect?.(row.dataset.cube || '', { focus: true });
    });
    listBody.addEventListener('dblclick', (event) => {
      const row = (event.target instanceof Element ? event.target : null)?.closest<HTMLElement>(
        '[data-cube]',
      );
      if (!row) return;
      handlers().onSelect?.(row.dataset.cube || '', { focus: false });
      handlers().onImport?.();
    });
    this.options.windowRef?.addEventListener?.('keydown', (event) => {
      if (!this.options.isActive()) return;
      const active = this.options.documentRef?.activeElement || null;
      const editable =
        (active instanceof HTMLInputElement ||
          active instanceof HTMLTextAreaElement ||
          active instanceof HTMLSelectElement) &&
        !('readOnly' in active && active.readOnly);
      if (editable) {
        if (event.key === 'Enter' && active === searchInput) {
          event.preventDefault();
          handlers().onImport?.();
        }
        return;
      }
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        handlers().onMoveSelection?.(event.key === 'ArrowDown' ? 1 : -1);
      } else if (event.key === 'Enter' && active === searchInput) {
        event.preventDefault();
        handlers().onImport?.();
      }
    });
    return {
      dialog,
      listBody,
      detailContainer,
      detailTitle,
      detailIcon,
      detailMeta,
      detailDescription,
      previewContainer,
      previewCanvas,
      previewStatus,
      favoriteButton,
      deleteButton,
      promoteButton,
      editButton,
      editSaveButton,
      editCancelButton,
      placeButton,
      versionControl: version.control,
      versionInput: version.input,
      versionToggle: version.toggle,
      versionListbox: version.listbox,
      searchInput,
      emptyState,
    };
  }
}
