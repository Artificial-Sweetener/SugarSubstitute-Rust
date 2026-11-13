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
/** Execute the shared pixel policy off the UI thread without editor or graph state. */
import { editMaskInput } from '../domain/MaskInputEditing.js';
import { readMaskEditingRequest } from './MaskEditingTransport.js';
import { isRecord } from '../../types/common.js';

/** Retain only the worker message boundary from the browser host. */
interface MaskWorkerScope {
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
  postMessage(value: unknown): void;
}

/** Install deterministic processing with explicit, attributed failure responses. */
export function installMaskEditingWorker(scope: MaskWorkerScope): void {
  scope.addEventListener('message', (event) => {
    const raw: unknown = event.data;
    const id = isRecord(raw) && typeof raw.id === 'number' ? raw.id : null;
    try {
      const request = readMaskEditingRequest(raw);
      scope.postMessage({
        id: request.id,
        result: editMaskInput(request.source, request.settings),
      });
    } catch (error: unknown) {
      scope.postMessage({
        id,
        error: error instanceof Error ? error.message : 'Mask preparation failed.',
      });
    }
  });
}

// Comfy imports every deployed .js extension in the page, including dedicated worker entries.
if (typeof document === 'undefined' && 'importScripts' in globalThis)
  installMaskEditingWorker(globalThis);
