/** @jest-environment node */
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
/** Prove the spelling gateway loads once, persists added words, and fails closed. */
import { jest } from '@jest/globals';
import { readFileSync } from 'node:fs';
import { SpellcheckModule } from '../../../frontend/comfyui/ui/prompt_editor/spellcheck/SpellcheckModule.js';
import {
  WorkerSpellcheckGateway,
  type SpellcheckPort,
} from '../../../frontend/comfyui/ui/prompt_editor/spellcheck/WorkerSpellcheckGateway.js';

const FILES = { module: 'module', aff: 'aff', dic: 'dic' };

/** A worker answered in this thread by `answer`, recording every request. */
function port(answer: (request: Record<string, unknown>) => unknown) {
  const listeners = new Map<string, ((event: never) => void)[]>();
  const requests: Record<string, unknown>[] = [];
  const terminate = jest.fn();
  const worker: SpellcheckPort = {
    postMessage(message) {
      const request = message as Record<string, unknown>;
      requests.push(request);
      queueMicrotask(() => {
        let reply: unknown;
        try {
          reply = { id: request.id, status: 'ready', value: answer(request) };
        } catch (error) {
          reply = { id: request.id, status: 'failed', message: String(error) };
        }
        for (const listener of listeners.get('message') ?? []) listener({ data: reply } as never);
      });
    },
    addEventListener(type: string, listener: (event: never) => void) {
      listeners.set(type, [...(listeners.get(type) ?? []), listener]);
    },
    terminate,
  };
  return { worker, requests, terminate };
}

test('the real module checks, suggests and accepts added words', async () => {
  const read = (name: string): Uint8Array<ArrayBuffer> =>
    new Uint8Array(readFileSync(`frontend/static/spellcheck/${name}`));
  const speller = await SpellcheckModule.load(
    read('sugarcubes-spellcheck.wasm'),
    read('en_US.aff'),
    read('en_US.dic'),
  );
  expect(speller.check(['garden', 'beautful', 'victorian', 'zorblax'])).toEqual([
    true,
    false,
    true,
    false,
  ]);
  expect(speller.suggest('beautful', 8)[0]).toBe('beautiful');
  expect(speller.add('zorblax')).toBe(true);
  expect(speller.check(['zorblax'])).toEqual([true]);
});

test('the dictionary loads on the first check, with the words the person added', async () => {
  const saved = ['zorblax'];
  const add = jest.fn(async (word: string) => void saved.push(word));
  const { worker, requests } = port((request) =>
    request.kind === 'check'
      ? (request.words as string[]).map((word) => word !== 'beautful')
      : true,
  );
  const gateway = new WorkerSpellcheckGateway(worker, FILES, { read: () => saved, add }, jest.fn());
  expect(requests).toEqual([]);
  const signal = new AbortController().signal;
  expect(await gateway.checkWords(['beautful', 'garden'], signal)).toEqual({
    status: 'ready',
    value: [false, true],
  });
  await gateway.checkWords(['garden'], signal);
  expect(requests.filter((request) => request.kind === 'load')).toEqual([
    expect.objectContaining({ load: { ...FILES, words: ['zorblax'] } }),
  ]);
  expect(await gateway.addToDictionary('qwertz', signal)).toEqual({ status: 'ready', value: true });
  expect(add).toHaveBeenCalledWith('qwertz');
});

test('a dictionary that cannot load reports spelling unavailable from then on', async () => {
  const report = jest.fn<(error: unknown) => void>();
  const { worker } = port((request) => {
    if (request.kind === 'load') throw new Error('missing dictionary');
    return [];
  });
  const gateway = new WorkerSpellcheckGateway(
    worker,
    FILES,
    { read: () => [], add: async () => undefined },
    report,
  );
  const signal = new AbortController().signal;
  expect(await gateway.checkWords(['garden'], signal)).toEqual({
    status: 'unavailable',
    code: 'dictionary-unavailable',
  });
  expect(gateway.availability()).toEqual({
    status: 'unavailable',
    code: 'dictionary-unavailable',
  });
  expect(report).toHaveBeenCalled();
});

test('an aborted question settles as cancelled without waiting for the worker', async () => {
  const { worker } = port(() => true);
  const gateway = new WorkerSpellcheckGateway(
    worker,
    FILES,
    { read: () => [], add: async () => undefined },
    jest.fn(),
  );
  const controller = new AbortController();
  controller.abort();
  expect(await gateway.suggest('beautful', 8, controller.signal)).toEqual({ status: 'cancelled' });
});
