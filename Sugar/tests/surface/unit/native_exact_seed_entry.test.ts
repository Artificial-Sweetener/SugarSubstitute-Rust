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
/** @jest-environment jsdom */
import { expect, jest, test } from '@jest/globals';
import {
  bindExactSeedEntry,
  exactSeedDigits,
} from '../../../frontend/comfyui/ui/surface/appearance/NativeExactSeedEntry.js';
import type { NativeNumericField } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeNumericField.js';

const MAXIMUM = 2 ** 64;
const SEED = '17066688399168319613';

test('only whole numbers above the exact range and within the field count as exact digits', () => {
  expect(exactSeedDigits(SEED, MAXIMUM)).toBe(SEED);
  expect(exactSeedDigits(' 17,066,688,399,168,319,613 ', MAXIMUM)).toBe(SEED);
  expect(exactSeedDigits('18446744073709551615', MAXIMUM)).toBe('18446744073709551615');
  expect(exactSeedDigits('9007199254740992', MAXIMUM)).toBe('9007199254740992');
  expect(exactSeedDigits('9007199254740991', MAXIMUM)).toBeNull();
  expect(exactSeedDigits('18446744073709551616', MAXIMUM)).toBeNull();
  expect(exactSeedDigits('9007199254740993', 9007199254740992)).toBeNull();
  expect(exactSeedDigits('1.7066688399168319e19', MAXIMUM)).toBeNull();
  expect(exactSeedDigits('-17066688399168319613', MAXIMUM)).toBeNull();
  expect(exactSeedDigits('<b>17066688399168319613</b>', MAXIMUM)).toBeNull();
});

/** Build a number box whose host keeps a number and rewrites its text from it. */
function box(initial: number) {
  const input = document.createElement('input');
  document.body.append(input);
  let value = initial;
  let recorded: string | null = null;
  const writeValue = jest.fn((next: number) => {
    value = next;
  });
  const write = jest.fn((digits: string) => {
    recorded = digits;
  });
  const numeric = {
    input,
    maximum: MAXIMUM,
    readValue: () => value,
    disabled: () => false,
    writeValue,
  } as unknown as NativeNumericField;
  const entry = bindExactSeedEntry(input, () => ({
    numeric,
    exact: {
      read: (held) => (recorded !== null && Number(recorded) === held ? recorded : null),
      write,
    },
  }));
  const type = (text: string): void => {
    input.focus();
    input.value = text;
    input.dispatchEvent(new Event('input'));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
  };
  return {
    input,
    entry,
    type,
    writeValue,
    write,
    hostRewrites: (): void => {
      input.value = String(value);
    },
    record: (digits: string | null): void => {
      recorded = digits;
    },
    recorded: () => recorded,
    value: () => value,
  };
}

test('a typed seed above the exact range is recorded as typed and shown as typed', () => {
  const seed = box(5);
  seed.type(SEED);
  expect(seed.recorded()).toBe(SEED);
  expect(seed.writeValue).toHaveBeenCalledWith(Number(SEED));
  seed.hostRewrites();
  expect(seed.input.value).not.toBe(SEED);
  seed.entry.synchronize();
  expect(seed.input.value).toBe(SEED);
});

test('an ordinary entry records nothing and leaves the number to its box', () => {
  const seed = box(Number(SEED));
  seed.record(SEED);
  seed.entry.synchronize();
  expect(seed.input.value).toBe(SEED);
  seed.type('1234');
  expect(seed.write).not.toHaveBeenCalled();
  expect(seed.writeValue).not.toHaveBeenCalled();
});

test('digits recorded for another value are never shown', () => {
  const seed = box(77);
  seed.hostRewrites();
  seed.record(SEED);
  seed.entry.synchronize();
  expect(seed.input.value).toBe('77');
});

test('a box left without typing records nothing, and a released box stops listening', () => {
  const seed = box(Number(SEED));
  seed.record(SEED);
  seed.input.focus();
  seed.input.blur();
  expect(seed.write).not.toHaveBeenCalled();
  seed.entry.release();
  seed.type(SEED);
  expect(seed.write).not.toHaveBeenCalled();
});
