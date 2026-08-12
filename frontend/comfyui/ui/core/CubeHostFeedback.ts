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
/** Own host feedback and import-history persistence. */

import type { StorageService } from './StorageService.js';
import type { ToastService } from './ToastService.js';

export type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

interface FeedbackLogger {
  warn(...values: unknown[]): void;
}

interface CubeHostFeedbackOptions {
  storage: StorageService | null;
  toast: ToastService | null;
  logger: FeedbackLogger;
  lastCubeStorageKeys: readonly string[];
}

/** Route host feedback and durable import history through one owner. */
export class CubeHostFeedback {
  readonly #storage: StorageService | null;
  readonly #toast: ToastService | null;
  readonly #logger: FeedbackLogger;
  readonly #lastCubeStorageKeys: readonly string[];

  constructor(options: CubeHostFeedbackOptions) {
    this.#storage = options.storage;
    this.#toast = options.toast;
    this.#logger = options.logger;
    this.#lastCubeStorageKeys = options.lastCubeStorageKeys;
  }

  /** Return a stable display message for an unknown failure. */
  readErrorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  /** Persist the last imported canonical Cube id once per configured key. */
  persistLastCubeId(value: unknown): void {
    if (value == null) return;
    const trimmed = typeof value === 'string' ? value.trim() : '';
    if (!trimmed) return;
    try {
      if (!this.#storage) return;
      const seen = new Set<string>();
      for (const key of this.#lastCubeStorageKeys) {
        if (!key || seen.has(key)) continue;
        this.#storage.writeValue(key, trimmed);
        seen.add(key);
      }
    } catch (error: unknown) {
      this.#logger.warn('SugarCubes: failed to persist the last imported Cube id.', error);
    }
  }

  /** Push one host-facing toast through the configured feedback service. */
  pushToast(severity: ToastSeverity, summary: string, detail: string): void {
    this.#toast?.push?.(severity, summary, detail);
  }
}
