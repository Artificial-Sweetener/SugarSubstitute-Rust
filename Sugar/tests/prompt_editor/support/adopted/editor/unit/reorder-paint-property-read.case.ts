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
/** Literal transport controls prove direct/fallback selection and raw failures; native proof owns actual CSSOM strings. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  readReorderPaintProperty,
  prepareReorderPaintStyle,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-style.ts';

test('direct named standard strings preserve empty strings and custom/unsupported fields use the original method', () => {
  const calls: string[] = [];
  const original = {
    font: 'italic 630 17.5px Georgia',
    'border-top': '1px solid rgb(44, 55, 66)',
    fill: 'rgb(11, 22, 33)',
    opacity: '0',
    color: '',
    'font-width': 42,
    '--sp-lora-height': 'wrong-expando',
    getPropertyValue(name: string): string {
      calls.push(name);
      return name === 'font-width' ? '87.5%' : name === '--sp-lora-height' ? '21px' : 'fallback';
    },
  } as unknown as CSSStyleDeclaration;
  assert.deepEqual(
    [
      readReorderPaintProperty(original, 'font'),
      readReorderPaintProperty(original, 'border-top'),
      readReorderPaintProperty(original, 'fill'),
      readReorderPaintProperty(original, 'opacity'),
      readReorderPaintProperty(original, 'color'),
    ],
    ['italic 630 17.5px Georgia', '1px solid rgb(44, 55, 66)', 'rgb(11, 22, 33)', '0', ''],
  );
  assert.equal(readReorderPaintProperty(original, 'font-width'), '87.5%');
  assert.equal(readReorderPaintProperty(original, '--sp-lora-height'), '21px');
  assert.equal(readReorderPaintProperty(original, 'unsupported-field'), 'fallback');
  assert.deepEqual(calls, ['font-width', '--sp-lora-height', 'unsupported-field']);
});

test('the sole paint loop retains exact order, URL exclusion and same-read literal/empty opacity', () => {
  const original = {
    'background-image': 'url("https://example.invalid/private.png")',
    color: 'rgb(11, 22, 33)',
    font: 'italic 630 17.5px Georgia',
    opacity: '0',
    getPropertyValue: () => '',
  } as unknown as CSSStyleDeclaration;
  assert.deepEqual(prepareReorderPaintStyle(original), {
    cssText:
      'background-image:none;opacity:0;color:rgb(11, 22, 33);font:italic 630 17.5px Georgia;',
    opacity: '0',
    display: '',
    position: '',
    transform: '',
    inset: '',
  });
  assert.deepEqual(
    prepareReorderPaintStyle({
      opacity: '',
      getPropertyValue: () => '',
    } as unknown as CSSStyleDeclaration),
    {
      cssText: '',
      opacity: '1',
      display: '',
      position: '',
      transform: '',
      inset: '',
    },
  );
});

test('raw direct-getter and fallback-method failures propagate without another attempted read', () => {
  for (const cause of [null, undefined]) {
    let fallbackCalls = 0;
    const direct = {
      get display(): never {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      },
      getPropertyValue() {
        fallbackCalls++;
        return '';
      },
    } as unknown as CSSStyleDeclaration;
    const fallback = {
      getPropertyValue(): never {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      },
    } as unknown as CSSStyleDeclaration;
    for (const original of [direct, fallback]) {
      let caught = false;
      try {
        readReorderPaintProperty(original, 'display');
      } catch (actual) {
        caught = true;
        assert.equal(actual, cause);
      }
      assert.equal(caught, true);
    }
    assert.equal(fallbackCalls, 0);
  }
});

/** The exact shorthand is reused from the sole native read, not another CSSOM query or parsed declarations. */
test('prepared inset witness retains the exact single-read resolved shorthand', () => {
  for (const inset of ['0px', '0px 4px 0px 0px', '', 'auto'] as const) {
    let reads = 0;
    const original = {
      get inset() {
        if (++reads > 1) throw new Error('inset-read-twice');
        return inset;
      },
      getPropertyValue: () => '',
    } as unknown as CSSStyleDeclaration;
    const prepared = prepareReorderPaintStyle(original);
    assert.equal(prepared.inset, inset);
    assert.equal(prepared.cssText, inset ? `inset:${inset};` : '');
    assert.equal(reads, 1);
    assert.equal(Object.isFrozen(prepared), true);
  }
});
