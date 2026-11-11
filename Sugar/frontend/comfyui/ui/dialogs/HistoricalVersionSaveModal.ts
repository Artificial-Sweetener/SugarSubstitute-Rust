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
 * Own historical-version save choice dialog behavior.
 */

import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';

interface HistoricalModalAdapter {
  getDocument?(): Document | null;
  getWindow?(): Window | null;
}

interface HistoricalVersionEntry {
  defaultAlias?: string;
  [key: string]: unknown;
}

export interface HistoricalVersionSaveOptions {
  entries?: HistoricalVersionEntry[];
}

/**
 * Coordinate the save-as-latest versus fork choice for historical revisions.
 */
export class HistoricalVersionSaveModal {
  private readonly shell: ModalShell;

  constructor({ adapter }: { adapter?: HistoricalModalAdapter | null } = {}) {
    this.shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-historical-save-overlay',
      dialogClassName: 'sugarcubes-historical-save-dialog',
    });
  }

  open({ entries = [] }: HistoricalVersionSaveOptions = {}): Promise<unknown> {
    const staleEntries = Array.isArray(entries) ? entries.filter(Boolean) : [];
    const message = buildHistoricalSaveMessage(staleEntries);
    const forkButton = $el('button.p-button.p-component.p-button-text.p-button-secondary', {
      type: 'button',
      textContent: 'Fork instead',
    }) as HTMLButtonElement;
    forkButton.addEventListener('click', () => this.shell.close('fork'));
    const result = this.shell.open({
      title: 'Save older version?',
      description: message,
      footerMeta: [forkButton],
      confirmLabel: 'Save as latest',
      cancelLabel: 'Cancel',
      cancelResult: null,
      onConfirm: () => this.shell.close('latest'),
      initialFocus: () => this.shell.elements.confirmButton,
    });
    return result;
  }

  close(result: unknown): void {
    this.shell.close(result);
  }
}

function buildHistoricalSaveMessage(entries: readonly HistoricalVersionEntry[]): string[] {
  if (entries.length > 1) {
    const names = entries
      .map((entry) => (typeof entry?.defaultAlias === 'string' ? entry.defaultAlias.trim() : ''))
      .filter(Boolean)
      .slice(0, 3);
    const suffix = names.length ? ` Included: ${names.join(', ')}.` : '';
    return [
      `${entries.length} SugarCubes were loaded from older versions.`,
      `Saving will apply these changes as new latest versions of their cubes.${suffix}`,
      'Fork instead if this work should stay separate.',
    ];
  }
  return [
    'This SugarCube was loaded from an older version.',
    'Saving will apply your changes as a new latest version of the cube.',
    'Fork instead if you want to keep this work separate.',
  ];
}
