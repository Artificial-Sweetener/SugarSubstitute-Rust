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
 * Own the SugarCubes core UI service layer in `web/comfyui/ui/core/ToastService.js`.
 */

import type { ComfyAdapter, HostToast } from './ComfyAdapter.js';

interface AlertDialog {
  alert(options: { title: string; message: string[]; confirmLabel: string }): Promise<unknown>;
}

interface ToastAdapter {
  getToast?(): HostToast | null;
  getConsole?(): {
    error?(...values: unknown[]): void;
    warn?(...values: unknown[]): void;
    info?(...values: unknown[]): void;
  } | null;
}

interface ToastServiceOptions {
  dialogs?: AlertDialog | null;
}

/**
 * Coordinate toast service behavior for the SugarCubes UI.
 */
export class ToastService {
  private readonly adapter: ToastAdapter;
  private readonly dialogs: AlertDialog | null;

  constructor(adapter: ComfyAdapter | ToastAdapter, { dialogs }: ToastServiceOptions = {}) {
    this.adapter = adapter;
    this.dialogs = dialogs || null;
  }

  push(severity: string, summary: string, detail: string): void {
    const toast = this.adapter?.getToast?.() || null;
    if (toast?.add) {
      toast.add({ severity, summary, detail, life: 5000 });
      return;
    }
    const message = detail ? `${summary}: ${detail}` : summary;
    const consoleRef = this.adapter?.getConsole?.() || null;
    if (severity === 'error') {
      consoleRef?.error?.(message);
      this.dialogs
        ?.alert?.({
          title: summary || 'SugarCubes error',
          message: detail ? [detail] : [],
          confirmLabel: 'OK',
        })
        ?.catch?.((error: unknown) => consoleRef?.error?.(error));
    } else if (severity === 'warn' || severity === 'warning') {
      consoleRef?.warn?.(message);
    } else {
      consoleRef?.info?.(message);
    }
  }
}
