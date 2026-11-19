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
/** Own host feedback. */

import type { ToastService } from './ToastService.js';

export type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

interface CubeHostFeedbackOptions {
  toast: ToastService | null;
}

/** Route host feedback through one owner. */
export class CubeHostFeedback {
  readonly #toast: ToastService | null;

  constructor(options: CubeHostFeedbackOptions) {
    this.#toast = options.toast;
  }

  /** Return a stable display message for an unknown failure. */
  readErrorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  /** Push one host-facing toast through the configured feedback service. */
  pushToast(severity: ToastSeverity, summary: string, detail: string): void {
    this.#toast?.push?.(severity, summary, detail);
  }

  /**
   * Show one sentence the desktop host has for the person, in the host's own words. A lasting
   * line stays until dismissed and answers whether the host's notifications showed it.
   */
  showHostLine(line: {
    readonly tone: 'info' | 'warn' | 'error';
    readonly text: string;
    readonly lasting: boolean;
  }): boolean {
    if (line.lasting) return this.pushLastingToast(line.tone, line.text, '');
    this.pushToast(line.tone, line.text, '');
    return true;
  }

  /** Push one toast that stays until dismissed; answer whether the host showed it. */
  pushLastingToast(severity: ToastSeverity, summary: string, detail: string): boolean {
    return this.#toast?.pushLasting(severity, summary, detail) ?? false;
  }
}
