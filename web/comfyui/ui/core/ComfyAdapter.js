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
/**
 * Coordinate comfy adapter behavior for the SugarCubes UI.
 */
export class ComfyAdapter {
    windowRef;
    documentRef;
    consoleRef;
    fetchRef;
    storageRef;
    app;
    api;
    liteGraph;
    alertRef;
    confirmRef;
    rafRef;
    cancelRafRef;
    setTimeoutRef;
    clearTimeoutRef;
    constructor(options = {}) {
        this.windowRef =
            options.window || (typeof window !== 'undefined' ? window : null);
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
                (typeof setTimeout !== 'undefined' ? setTimeout : null);
        this.clearTimeoutRef =
            options.clearTimeout ||
                this.windowRef?.clearTimeout ||
                (typeof clearTimeout !== 'undefined' ? clearTimeout : null);
    }
    getCanvas() {
        const appRef = this.getApp();
        return appRef?.canvas ?? appRef?.graph?.canvas ?? null;
    }
    getGraph() {
        return this.getApp()?.graph ?? null;
    }
    getToast() {
        return this.windowRef?.comfyAPI?.vueApp?.config?.globalProperties?.$toast || null;
    }
    getApp() {
        return this.app || this.windowRef?.app || null;
    }
    getApi() {
        return this.api || this.windowRef?.api || null;
    }
    getLiteGraph() {
        return this.liteGraph;
    }
    getWindow() {
        return this.windowRef;
    }
    getDocument() {
        return this.documentRef;
    }
    getConsole() {
        return this.consoleRef;
    }
    getFetch() {
        return this.fetchRef;
    }
    getStorage() {
        return this.storageRef;
    }
    getAlert() {
        return this.alertRef;
    }
    getConfirm() {
        return this.confirmRef;
    }
    getRaf() {
        return this.rafRef;
    }
    getCancelRaf() {
        return this.cancelRafRef;
    }
    getSetTimeout() {
        return this.setTimeoutRef;
    }
    getClearTimeout() {
        return this.clearTimeoutRef;
    }
}
