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
 * Own the SugarCubes dialog presentation layer in `frontend/comfyui/ui/dialogs/VersionDialog.js`.
 */

import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
import type { ModalAdapter, ModalShellElements } from './ModalShell.js';

interface VersionDialogStorage {
  readValue(key: string): string | null;
  writeValue(key: string, value: unknown): void;
}

export interface VersionSuggestion {
  default_alias?: string;
  current_version?: string;
  suggested_version?: string;
  reason?: string;
}

interface VersionDialogElements extends ModalShellElements {
  dismissCheckbox?: HTMLInputElement;
}

const CUBE_VERSION_PROMPT_KEY = 'sugarcubes.version_prompt.dismissed';

/**
 * Coordinate version dialog behavior for the SugarCubes UI.
 */
export class VersionDialog {
  private readonly storage: VersionDialogStorage | null;
  private readonly shell: ModalShell;
  elements: VersionDialogElements;

  constructor({
    adapter,
    storage,
  }: { adapter?: ModalAdapter | null; storage?: VersionDialogStorage | null } = {}) {
    this.storage = storage || null;
    this.shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-version-overlay',
      dialogClassName: 'sugarcubes-version-dialog',
    });
    this.elements = this.shell.elements;
  }

  isDismissed(): boolean {
    try {
      if (!this.storage) {
        return false;
      }
      return this.storage.readValue(CUBE_VERSION_PROMPT_KEY) === 'true';
    } catch (_error) {
      return false;
    }
  }

  setDismissed(value: boolean): void {
    try {
      if (!this.storage) {
        return;
      }
      this.storage.writeValue(CUBE_VERSION_PROMPT_KEY, value ? 'true' : 'false');
    } catch (_error) {
      return;
    }
  }

  open(suggestions: VersionSuggestion[] | null | undefined): Promise<boolean> {
    if (!Array.isArray(suggestions) || !suggestions.length) {
      return Promise.resolve(false);
    }
    if (this.isDismissed()) {
      return Promise.resolve(false);
    }
    const intro = $el(
      'p',
      'We detected manual version edits. Suggested versions are shown below based on cube changes.',
    );
    const list = $el('ul');
    suggestions.forEach((entry) => {
      const defaultAlias = entry?.default_alias || 'Unknown cube';
      const currentVersion = entry?.current_version || 'unknown';
      const suggested = entry?.suggested_version || 'unknown';
      const reason = entry?.reason || 'Update recommended';
      const item = $el('li', `${defaultAlias}: ${currentVersion} -> ${suggested} (${reason})`);
      list.appendChild(item);
    });
    const dismissCheckbox = $el('input', { type: 'checkbox' }) as HTMLInputElement;
    const dismissLabel = $el('label.sugarcubes-version-dialog__dismiss', [
      dismissCheckbox,
      $el('span', "Don't show again"),
    ]);
    const promise = this.shell.open({
      title: 'Version suggestions',
      body: [intro, list],
      footerMeta: dismissLabel,
      confirmLabel: 'OK',
      cancelLabel: 'Close',
      showCancel: false,
      cancelResult: false,
      onConfirm: () => {
        if (dismissCheckbox.checked) {
          this.setDismissed(true);
        }
        this.shell.close(true);
      },
      initialFocus: () => this.shell.elements.confirmButton,
    });
    this.elements = { ...this.shell.elements, dismissCheckbox };
    return promise as Promise<boolean>;
  }

  close(): void {
    if (this.elements.dismissCheckbox?.checked) {
      this.setDismissed(true);
    }
    this.shell.close(true);
  }
}
