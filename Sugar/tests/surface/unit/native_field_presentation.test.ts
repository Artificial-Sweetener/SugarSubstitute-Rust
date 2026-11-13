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
/** Prove field replacement visibility is independent of mount order and nested suppression. */
import { afterEach, expect, test } from '@jest/globals';
import {
  registerNativeFieldEnhancement,
  setNativeFieldSuppressed,
} from '../../../frontend/comfyui/ui/surface/NativeFieldPresentation.js';

afterEach(() => document.body.replaceChildren());

test.each([true, false])(
  'shares suppression across a native control and its enhancement (%s)',
  (suppressFirst) => {
    const row = document.createElement('div');
    const frame = document.createElement('div');
    const anchor = document.createElement('div');
    const replacement = document.createElement('div');
    frame.append(anchor);
    row.append(frame, replacement);
    document.body.append(row);
    if (suppressFirst) setNativeFieldSuppressed(frame, true);
    const release = registerNativeFieldEnhancement(anchor, replacement);
    setNativeFieldSuppressed(frame, true);
    expect(replacement.hidden).toBe(true);
    setNativeFieldSuppressed(row, true);
    setNativeFieldSuppressed(frame, false);
    expect(replacement.hidden).toBe(true);
    setNativeFieldSuppressed(row, false);
    expect(replacement.hidden).toBe(false);
    release();
    expect(anchor.hasAttribute('data-sc-model-native')).toBe(false);
  },
);

test.each([true, false])(
  'rejects competing enhancements and keeps stale disposal from releasing a new owner (reuse=%s)',
  (reuse) => {
    const anchor = document.createElement('div');
    const first = document.createElement('div');
    const second = reuse ? first : document.createElement('div');
    const releaseFirst = registerNativeFieldEnhancement(anchor, first);
    expect(() => registerNativeFieldEnhancement(anchor, second)).toThrow(
      'already owns an enhancement',
    );
    releaseFirst();
    const releaseSecond = registerNativeFieldEnhancement(anchor, second);
    releaseFirst();
    setNativeFieldSuppressed(anchor, true);
    expect(second.hidden).toBe(true);
    expect(anchor.hasAttribute('data-sc-model-native')).toBe(true);
    releaseSecond();
    expect(second.hidden).toBe(false);
  },
);
