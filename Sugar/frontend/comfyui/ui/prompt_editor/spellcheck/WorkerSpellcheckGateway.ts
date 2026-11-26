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
/** The prompt editor's spelling gateway, answered by the spellchecker's Web Worker. */
import type {
  SpellcheckGateway,
  SpellingAvailability,
  SpellingLookup,
} from '../../../../sugarprompt/services/src/index.js';
import { spellcheckReply, type SpellcheckLoad } from './SpellcheckProtocol.js';

/** The worker as the gateway uses it. */
export interface SpellcheckPort {
  postMessage(message: unknown): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: 'error', listener: (event: Event) => void): void;
  terminate(): void;
}

/** The words the person added to the dictionary, kept between pages. */
export interface PersonalSpellingWords {
  read(): readonly string[];
  add(word: string): Promise<void>;
}

/**
 * Ask the worker for checks and suggestions. The dictionary loads on the first check, so a page
 * whose prompts are never checked never downloads it. A worker that fails to load reports the
 * dictionary unavailable from then on, and the editor shows no spelling marks.
 */
export class WorkerSpellcheckGateway implements SpellcheckGateway {
  readonly identity = Symbol('SugarCubes spellchecker');
  readonly languageTag = 'en-US';
  readonly #port: SpellcheckPort;
  readonly #files: Omit<SpellcheckLoad, 'words'>;
  readonly #words: PersonalSpellingWords;
  readonly #report: (error: unknown) => void;
  readonly #pending = new Map<number, (reply: { ok: boolean; value: unknown }) => void>();
  #next = 0;
  #loaded: Promise<boolean> | null = null;
  #failed = false;

  constructor(
    port: SpellcheckPort,
    files: Omit<SpellcheckLoad, 'words'>,
    words: PersonalSpellingWords,
    report: (error: unknown) => void,
  ) {
    this.#port = port;
    this.#files = files;
    this.#words = words;
    this.#report = report;
    port.addEventListener('message', (event) => {
      const reply = spellcheckReply(event.data);
      const settle = reply ? this.#pending.get(reply.id) : undefined;
      if (!reply || !settle) return;
      this.#pending.delete(reply.id);
      if (reply.status === 'failed') this.#report(new Error(reply.message));
      settle({
        ok: reply.status === 'ready',
        value: reply.status === 'ready' ? reply.value : null,
      });
    });
    port.addEventListener('error', (event) => {
      this.#failed = true;
      this.#report(new Error(`The spellchecker stopped: ${event.type}`));
      for (const settle of this.#pending.values()) settle({ ok: false, value: null });
      this.#pending.clear();
    });
  }

  availability(): SpellingAvailability {
    return this.#failed
      ? { status: 'unavailable', code: 'dictionary-unavailable' }
      : { status: 'available', dictionaryAdd: true, sessionIgnore: false };
  }

  async checkWords(words: readonly string[], signal: AbortSignal): Promise<SpellingLookup> {
    return this.#ask(signal, { kind: 'check', words: [...words] });
  }

  async suggest(word: string, limit: 8, signal: AbortSignal): Promise<SpellingLookup> {
    return this.#ask(signal, { kind: 'suggest', word, limit });
  }

  async addToDictionary(word: string, signal: AbortSignal): Promise<SpellingLookup> {
    const outcome = await this.#ask(signal, { kind: 'add', word });
    if (outcome.status === 'ready' && outcome.value === true) await this.#words.add(word);
    return outcome;
  }

  /** Stop the worker; later questions are answered unavailable. */
  dispose(): void {
    this.#failed = true;
    this.#port.terminate();
    for (const settle of this.#pending.values()) settle({ ok: false, value: null });
    this.#pending.clear();
  }

  async #ask(signal: AbortSignal, request: Record<string, unknown>): Promise<SpellingLookup> {
    if (signal.aborted) return { status: 'cancelled' };
    if (!(await this.#load()) || this.#failed)
      return { status: 'unavailable', code: 'dictionary-unavailable' };
    if (signal.aborted) return { status: 'cancelled' };
    const reply = await this.#send(request, signal);
    if (reply === 'cancelled') return { status: 'cancelled' };
    return reply.ok
      ? { status: 'ready', value: reply.value }
      : { status: 'unavailable', code: 'backend-unavailable' };
  }

  /** Load the dictionary once; every question waits for it. */
  #load(): Promise<boolean> {
    this.#loaded ??= this.#send(
      { kind: 'load', load: { ...this.#files, words: this.#words.read() } },
      null,
    ).then((reply) => {
      const loaded = reply !== 'cancelled' && reply.ok;
      if (!loaded) this.#failed = true;
      return loaded;
    });
    return this.#loaded;
  }

  #send(
    request: Record<string, unknown>,
    signal: AbortSignal | null,
  ): Promise<{ ok: boolean; value: unknown } | 'cancelled'> {
    const id = ++this.#next;
    return new Promise((resolve) => {
      const abort = (): void => {
        // The worker still answers; its reply is no longer wanted.
        this.#pending.delete(id);
        resolve('cancelled');
      };
      signal?.addEventListener('abort', abort, { once: true });
      this.#pending.set(id, (reply) => {
        signal?.removeEventListener('abort', abort);
        resolve(reply);
      });
      this.#port.postMessage({ id, ...request });
    });
  }
}
