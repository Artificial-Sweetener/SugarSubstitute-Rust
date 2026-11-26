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
 * The spellchecker module built from `spellcheck/`: one loaded Hunspell dictionary, asked through
 * the module's C ABI. Strings are written into module memory as UTF-8 and answers are read back
 * from its output buffer, which stays valid only until the next call.
 */

/** The functions the module exports, checked before any is called. */
interface SpellcheckExports {
  readonly memory: WebAssembly.Memory;
  spell_alloc(length: number): number;
  spell_free(pointer: number, length: number): void;
  spell_load(affPointer: number, affLength: number, dicPointer: number, dicLength: number): number;
  spell_check_words(pointer: number, length: number): number;
  spell_suggest(pointer: number, length: number, limit: number): number;
  spell_add(pointer: number, length: number): number;
  spell_output(): number;
}

const FUNCTIONS = [
  'spell_alloc',
  'spell_free',
  'spell_load',
  'spell_check_words',
  'spell_suggest',
  'spell_add',
  'spell_output',
] as const;

/** Narrow a module's exports to the spellchecker's, or explain what is missing. */
function spellcheckExports(exports: WebAssembly.Exports): SpellcheckExports {
  if (!(exports.memory instanceof WebAssembly.Memory))
    throw new TypeError('The spellchecker module exports no memory.');
  for (const name of FUNCTIONS)
    if (typeof exports[name] !== 'function')
      throw new TypeError(`The spellchecker module does not export ${name}.`);
  return exports as unknown as SpellcheckExports;
}

/** One dictionary loaded into one module instance. */
export class SpellcheckModule {
  readonly #exports: SpellcheckExports;
  readonly #encoder = new TextEncoder();
  readonly #decoder = new TextDecoder();

  private constructor(exports: SpellcheckExports) {
    this.#exports = exports;
  }

  /** Instantiate the module and load the dictionary's affix and word files into it. */
  static async load(
    module: Uint8Array<ArrayBuffer>,
    aff: Uint8Array,
    dic: Uint8Array,
  ): Promise<SpellcheckModule> {
    const { instance } = await WebAssembly.instantiate(module, {});
    const speller = new SpellcheckModule(spellcheckExports(instance.exports));
    const loaded = speller.#with(aff, (affPointer) =>
      speller.#with(dic, (dicPointer) =>
        speller.#exports.spell_load(affPointer, aff.length, dicPointer, dic.length),
      ),
    );
    if (loaded !== 0) throw new Error('The spelling dictionary could not be read.');
    return speller;
  }

  /** Whether each word is spelled correctly, in order. */
  check(words: readonly string[]): boolean[] {
    if (!words.length) return [];
    // Words never hold a line break, so one call checks them all.
    const count = this.#call(words.join('\n'), (pointer, length) =>
      this.#exports.spell_check_words(pointer, length),
    );
    if (count !== words.length) throw new Error('The spellchecker did not answer every word.');
    return [...this.#output(count)].map((answer) => answer === 0x31);
  }

  /** Up to `limit` corrections for one word, best first. */
  suggest(word: string, limit: number): string[] {
    const length = this.#call(word, (pointer, bytes) =>
      this.#exports.spell_suggest(pointer, bytes, limit),
    );
    if (length < 0) throw new Error('The spellchecker could not suggest corrections.');
    const text = this.#decoder.decode(this.#output(length));
    return text ? text.split('\n') : [];
  }

  /** Accept a word from now on; false when the dictionary refused it. */
  add(word: string): boolean {
    return this.#call(word, (pointer, length) => this.#exports.spell_add(pointer, length)) === 0;
  }

  #call(text: string, run: (pointer: number, length: number) => number): number {
    const bytes = this.#encoder.encode(text);
    const answer = this.#with(bytes, (pointer) => run(pointer, bytes.length));
    if (answer < 0) throw new Error('The spellchecker refused the text it was given.');
    return answer;
  }

  /** Copy `bytes` into module memory for one call, then release them. */
  #with(bytes: Uint8Array, run: (pointer: number) => number): number {
    const length = Math.max(bytes.length, 1);
    const pointer = this.#exports.spell_alloc(length);
    try {
      new Uint8Array(this.#exports.memory.buffer, pointer, bytes.length).set(bytes);
      return run(pointer);
    } finally {
      this.#exports.spell_free(pointer, length);
    }
  }

  /** The last answer's bytes, copied out before module memory can grow or be rewritten. */
  #output(length: number): Uint8Array {
    return new Uint8Array(
      this.#exports.memory.buffer,
      this.#exports.spell_output(),
      length,
    ).slice();
  }
}
