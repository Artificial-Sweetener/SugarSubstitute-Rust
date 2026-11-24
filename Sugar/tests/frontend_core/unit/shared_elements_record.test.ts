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
/** The page and the native host draw shared elements from one record; the page's copy matches it. */
import { existsSync, readFileSync } from 'node:fs';
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  CAPTION_BUTTON,
  WORKFLOW_VIEW_ICON,
  captionButtonCss,
  sharedIconUrl,
} from '../../../frontend/comfyui/ui/core/SharedElements.js';
import { ConsoleCaptionButton } from '../../../frontend/comfyui/ui/console/ConsoleCaptionButton.js';

interface ElementRecord {
  captionButton: {
    width: number;
    height: number;
    glyph: number;
    fill: Record2<'dark' | 'light', 'rest' | 'hover' | 'pressed'>;
  };
  toggles: { console: Toggle; outputCanvas: Toggle };
  windowClose: string;
  workflowView: typeof WORKFLOW_VIEW_ICON;
}
type Record2<A extends string, B extends string> = Record<A, Record<B, number[]>>;
interface Toggle {
  toOpen: string;
  toClose: string;
}

const recordUrl = new URL('../../../integrations/contracts/ui/elements.json', import.meta.url);
const record = JSON.parse(readFileSync(recordUrl, 'utf8')) as ElementRecord;

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

test('the caption button the page draws is the shared record’s', () => {
  const { width, height, glyph, fill } = record.captionButton;
  expect([CAPTION_BUTTON.width, CAPTION_BUTTON.height, CAPTION_BUTTON.glyph]).toEqual([
    width,
    height,
    glyph,
  ]);
  expect(CAPTION_BUTTON.hover).toBe(fill.dark.hover[3]);
  expect(CAPTION_BUTTON.pressed).toBe(fill.dark.pressed[3]);
  expect(fill.light.hover[3]).toBe(fill.dark.hover[3]);
  expect(fill.dark.rest[3]).toBe(0);
  const css = captionButtonCss('.probe');
  expect(css).toContain(`width:${String(width)}px; height:${String(height)}px`);
  expect(css).toContain(`currentColor ${((fill.dark.hover[3] / 255) * 100).toFixed(2)}%`);
  expect(css).toContain(`currentColor ${((fill.dark.pressed[3] / 255) * 100).toFixed(2)}%`);
  expect(css).toContain('border-radius:0');
});

test('the graph/masonry artwork is the shared record’s', () => {
  const { outline, cornerRadius, link, durationMs, linkRetract, graph, masonry, swing } =
    record.workflowView;
  expect(WORKFLOW_VIEW_ICON).toEqual({
    outline,
    cornerRadius,
    link,
    durationMs,
    linkRetract,
    graph,
    masonry,
    swing,
  });
});

test('every shared icon the page serves is one the record names and the build ships', () => {
  const named = [
    record.toggles.console.toOpen,
    record.toggles.console.toClose,
    record.toggles.outputCanvas.toOpen,
    record.toggles.outputCanvas.toClose,
    record.windowClose,
  ];
  for (const file of named) {
    const name = file.replace(/^icons\//, '').replace(/\.svg$/, '') as Parameters<
      typeof sharedIconUrl
    >[0];
    expect(sharedIconUrl(name)).toMatch(new RegExp(`/icons/shared/${name}\\.svg$`));
    expect(existsSync(new URL(`../../../integrations/contracts/ui/${file}`, import.meta.url))).toBe(
      true,
    );
  }
});

test('the console button sits in the frame’s row only, and shows where it goes', async () => {
  const toggle = jest.fn();
  const button = new ConsoleCaptionButton(document, toggle);
  button.setup();
  expect(document.querySelector('.sc-console-caption')).toBeNull();
  const frame = document.createElement('header');
  frame.className = 'sc-substitute-frame';
  document.body.append(frame);
  await Promise.resolve();
  const element = frame.querySelector<HTMLButtonElement>('.sc-console-caption');
  if (!element) throw new Error('Console button did not join the frame row');
  expect(frame.hasAttribute('data-console-caption')).toBe(true);
  const icon = (): string => element.firstElementChild?.getAttribute('style') ?? '';
  expect(icon()).toContain('console-open.svg');
  expect(element.getAttribute('aria-pressed')).toBe('false');
  button.update(true);
  expect(icon()).toContain('console-close.svg');
  expect(element.getAttribute('aria-pressed')).toBe('true');
  expect(element.title).toBe('Hide console');
  element.click();
  expect(toggle).toHaveBeenCalledTimes(1);
  frame.remove();
  await Promise.resolve();
  expect(document.querySelector('.sc-console-caption')).toBeNull();
  button.dispose();
});
