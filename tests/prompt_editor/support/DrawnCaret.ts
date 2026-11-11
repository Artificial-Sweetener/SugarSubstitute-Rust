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
/** Read the caret the prompt editor draws, which is the only caret it paints. */
import type { Page } from '@playwright/test';

/** Viewport rectangle of the drawn primary caret. */
export interface DrawnCaretRect {
  readonly x: number;
  readonly top: number;
  readonly bottom: number;
  readonly width: number;
}

/** Read the drawn primary caret of the mounted prompt, or null while none is drawn. */
export async function drawnCaret(page: Page): Promise<DrawnCaretRect | null> {
  return page.evaluate(() => {
    const cursor = document.querySelector('[data-sc-prompt-editor] .cm-cursor-primary');
    if (!cursor) return null;
    const rect = cursor.getBoundingClientRect();
    return rect.height > 0
      ? { x: rect.left, top: rect.top, bottom: rect.bottom, width: rect.width }
      : null;
  });
}

/** Wait until the drawn caret sits on the visual row spanning `top` to `bottom`. */
export async function expectDrawnCaretOnRow(
  page: Page,
  row: { readonly top: number; readonly bottom: number },
): Promise<void> {
  await page.waitForFunction(
    ({ top, bottom }) => {
      const cursor = document.querySelector('[data-sc-prompt-editor] .cm-cursor-primary');
      if (!cursor) return false;
      const rect = cursor.getBoundingClientRect();
      const middle = (rect.top + rect.bottom) / 2;
      return rect.height > 0 && middle >= top && middle <= bottom;
    },
    { top: row.top, bottom: row.bottom },
    { timeout: 2_000 },
  );
}
