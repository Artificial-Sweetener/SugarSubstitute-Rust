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
 * Own the SugarCubes core UI service layer in `frontend/comfyui/ui/core/StorageService.js`.
 */
import { isRecord } from '../types/common.js';
/**
 * Coordinate storage service behavior for the SugarCubes UI.
 */
export class StorageService {
    storage;
    constructor(adapter) {
        this.storage = adapter?.getStorage?.() || null;
    }
    /** Return the host storage boundary for migration-oriented consumers. */
    getStorage() {
        return this.storage;
    }
    readList(key) {
        if (!this.storage)
            return [];
        try {
            const raw = this.storage.getItem(key);
            const parsed = raw ? JSON.parse(raw) : [];
            return Array.isArray(parsed)
                ? parsed.filter((entry) => typeof entry === 'string' && Boolean(entry))
                : [];
        }
        catch (_error) {
            return [];
        }
    }
    writeList(key, values) {
        if (!this.storage)
            return;
        try {
            const payload = Array.isArray(values)
                ? values.filter((entry) => typeof entry === 'string' && Boolean(entry))
                : [];
            this.storage.setItem(key, JSON.stringify(payload));
        }
        catch (_error) {
            return;
        }
    }
    readSet(key) {
        if (!this.storage)
            return new Set();
        try {
            const raw = this.storage.getItem(key);
            const parsed = raw ? JSON.parse(raw) : [];
            return new Set(Array.isArray(parsed)
                ? parsed.filter((entry) => typeof entry === 'string' && Boolean(entry))
                : []);
        }
        catch (_error) {
            return new Set();
        }
    }
    writeSet(key, setValue) {
        if (!this.storage)
            return;
        try {
            const values = Array.from(setValue ?? []).filter(Boolean);
            this.storage.setItem(key, JSON.stringify(values));
        }
        catch (_error) {
            return;
        }
    }
    readJson(key) {
        if (!this.storage)
            return null;
        try {
            const raw = this.storage.getItem(key);
            const parsed = raw ? JSON.parse(raw) : null;
            return isRecord(parsed) ? parsed : null;
        }
        catch (_error) {
            return null;
        }
    }
    writeJson(key, value) {
        if (!this.storage)
            return;
        try {
            this.storage.setItem(key, JSON.stringify(value));
        }
        catch (_error) {
            return;
        }
    }
    readValue(key) {
        if (!this.storage)
            return null;
        try {
            return this.storage.getItem(key);
        }
        catch (_error) {
            return null;
        }
    }
    writeValue(key, value) {
        if (!this.storage)
            return;
        try {
            if (value == null) {
                return;
            }
            this.storage.setItem(key, String(value));
        }
        catch (_error) {
            return;
        }
    }
}
