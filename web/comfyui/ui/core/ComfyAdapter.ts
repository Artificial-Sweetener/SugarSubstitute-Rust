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
 * Own the SugarCubes core UI service layer in `web/comfyui/ui/core/ComfyAdapter.js`.
 */

import type { ComfyApplication, ComfyCanvas, ComfyGraph, ComfyHostApi } from '../types/graph.js';

export interface HostToast {
  add(message: { severity: string; summary: string; detail: string; life: number }): void;
}

export type BrowserSetTimeout = (
  handler: TimerHandler,
  timeout?: number,
  ...args: unknown[]
) => number;
export type BrowserClearTimeout = (id?: number) => void;

export interface HostWindow extends Window {
  app?: ComfyApplication;
  api?: ComfyHostApi;
  comfyAPI?: {
    vueApp?: {
      config?: {
        globalProperties?: { $toast?: HostToast };
      };
    };
  };
}

export interface ComfyAdapterOptions {
  window?: HostWindow | null;
  document?: Document | null;
  console?: Console | null;
  fetch?: typeof fetch | null;
  storage?: Storage | null;
  app?: ComfyApplication | null;
  api?: ComfyHostApi | null;
  liteGraph?: LiteGraphHost | null;
  alert?: typeof alert | null;
  confirm?: typeof confirm | null;
  raf?: typeof requestAnimationFrame | null;
  cancelRaf?: typeof cancelAnimationFrame | null;
  setTimeout?: BrowserSetTimeout | null;
  clearTimeout?: BrowserClearTimeout | null;
}

/**
 * Coordinate comfy adapter behavior for the SugarCubes UI.
 */
export class ComfyAdapter {
  private readonly windowRef: HostWindow | null;
  private readonly documentRef: Document | null;
  private readonly consoleRef: Console | null;
  private readonly fetchRef: typeof fetch | null;
  private readonly storageRef: Storage | null;
  private readonly app: ComfyApplication | null;
  private readonly api: ComfyHostApi | null;
  private readonly liteGraph: LiteGraphHost | null;
  private readonly alertRef: typeof alert | null;
  private readonly confirmRef: typeof confirm | null;
  private readonly rafRef: typeof requestAnimationFrame | null;
  private readonly cancelRafRef: typeof cancelAnimationFrame | null;
  private readonly setTimeoutRef: BrowserSetTimeout | null;
  private readonly clearTimeoutRef: BrowserClearTimeout | null;

  constructor(options: ComfyAdapterOptions = {}) {
    this.windowRef =
      options.window || (typeof window !== 'undefined' ? (window as HostWindow) : null);
    this.documentRef =
      options.document ||
      this.windowRef?.document ||
      (typeof document !== 'undefined' ? document : null);
    this.consoleRef = options.console || (typeof console !== 'undefined' ? console : null);
    this.fetchRef = options.fetch || (typeof fetch !== 'undefined' ? fetch : null);
    this.storageRef =
      options.storage ||
      this.windowRef?.localStorage ||
      (typeof localStorage !== 'undefined' ? localStorage : null);
    this.app = options.app || this.windowRef?.app || null;
    this.api = options.api || this.windowRef?.api || null;
    this.liteGraph = options.liteGraph || (typeof LiteGraph !== 'undefined' ? LiteGraph : null);
    this.alertRef =
      options.alert || this.windowRef?.alert || (typeof alert !== 'undefined' ? alert : null);
    this.confirmRef =
      options.confirm ||
      this.windowRef?.confirm ||
      (typeof confirm !== 'undefined' ? confirm : null);
    this.rafRef =
      options.raf ||
      this.windowRef?.requestAnimationFrame ||
      (typeof requestAnimationFrame !== 'undefined' ? requestAnimationFrame : null);
    this.cancelRafRef =
      options.cancelRaf ||
      this.windowRef?.cancelAnimationFrame ||
      (typeof cancelAnimationFrame !== 'undefined' ? cancelAnimationFrame : null);
    this.setTimeoutRef =
      options.setTimeout ||
      this.windowRef?.setTimeout ||
      (typeof setTimeout !== 'undefined' ? (setTimeout as BrowserSetTimeout) : null);
    this.clearTimeoutRef =
      options.clearTimeout ||
      this.windowRef?.clearTimeout ||
      (typeof clearTimeout !== 'undefined' ? (clearTimeout as BrowserClearTimeout) : null);
  }

  getCanvas(): ComfyCanvas | null {
    const appRef = this.getApp();
    return appRef?.canvas ?? appRef?.graph?.canvas ?? null;
  }

  getGraph(): ComfyGraph | null {
    return this.getApp()?.graph ?? null;
  }

  getToast(): HostToast | null {
    return this.windowRef?.comfyAPI?.vueApp?.config?.globalProperties?.$toast || null;
  }

  getApp(): ComfyApplication | null {
    return this.app || this.windowRef?.app || null;
  }

  getApi(): ComfyHostApi | null {
    return this.api || this.windowRef?.api || null;
  }

  getLiteGraph(): LiteGraphHost | null {
    return this.liteGraph;
  }

  getWindow(): HostWindow | null {
    return this.windowRef;
  }

  getDocument(): Document | null {
    return this.documentRef;
  }

  getConsole(): Console | null {
    return this.consoleRef;
  }

  getFetch(): typeof fetch | null {
    return this.fetchRef;
  }

  getStorage(): Storage | null {
    return this.storageRef;
  }

  getAlert(): typeof alert | null {
    return this.alertRef;
  }

  getConfirm(): typeof confirm | null {
    return this.confirmRef;
  }

  getRaf(): typeof requestAnimationFrame | null {
    return this.rafRef;
  }

  getCancelRaf(): typeof cancelAnimationFrame | null {
    return this.cancelRafRef;
  }

  getSetTimeout(): BrowserSetTimeout | null {
    return this.setTimeoutRef;
  }

  getClearTimeout(): BrowserClearTimeout | null {
    return this.clearTimeoutRef;
  }
}
