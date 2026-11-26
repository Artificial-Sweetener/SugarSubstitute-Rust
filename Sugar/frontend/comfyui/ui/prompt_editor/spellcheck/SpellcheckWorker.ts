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
 * The spellchecker's Web Worker: it loads the module and the dictionary once and answers checks
 * and suggestions off the page's thread, so typing never waits for the dictionary.
 */
import { SpellcheckModule } from './SpellcheckModule.js';
import {
  spellcheckRequest,
  type SpellcheckLoad,
  type SpellcheckRequest,
} from './SpellcheckProtocol.js';

/** The worker's message port, as a dedicated worker's global scope offers it. */
interface WorkerScope {
  postMessage(message: unknown): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
}

const scope = globalThis as unknown as WorkerScope;
let speller: Promise<SpellcheckModule> | null = null;

/** Read one file the page named as bytes. */
async function bytes(url: string): Promise<Uint8Array<ArrayBuffer>> {
  const response = await fetch(url);
  if (!response.ok)
    throw new Error(`The spellchecker could not read ${url} (${String(response.status)}).`);
  return new Uint8Array(await response.arrayBuffer());
}

/** Load the module, the dictionary and the person's own words. */
async function load(files: SpellcheckLoad): Promise<SpellcheckModule> {
  const [module, aff, dic] = await Promise.all([
    bytes(files.module),
    bytes(files.aff),
    bytes(files.dic),
  ]);
  const loaded = await SpellcheckModule.load(module, aff, dic);
  for (const word of files.words) loaded.add(word);
  return loaded;
}

/** Answer one request. */
async function answer(request: SpellcheckRequest): Promise<unknown> {
  if (request.kind === 'load') {
    speller ??= load(request.load);
    await speller;
    return true;
  }
  if (!speller) throw new Error('The spellchecker was asked before it was loaded.');
  const ready = await speller;
  switch (request.kind) {
    case 'check':
      return ready.check(request.words);
    case 'suggest':
      return ready.suggest(request.word, request.limit);
    case 'add':
      return ready.add(request.word);
  }
}

scope.addEventListener('message', (event) => {
  const request = spellcheckRequest(event.data);
  if (!request) return;
  answer(request).then(
    (value) => scope.postMessage({ id: request.id, status: 'ready', value }),
    (error: unknown) =>
      scope.postMessage({
        id: request.id,
        status: 'failed',
        message: error instanceof Error ? error.message : String(error),
      }),
  );
});
