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
/**
 * Prove rich editing, decoded banners and Alt dragging through both renderer adapters.
 * @jest-environment node
 */
import { createServer, type Server } from 'node:http';
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';
import type {} from '../support/BrowserProof.js';
import { closeBrowserFixture } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let server: Server;
let browser: Browser;
let address: string;
const source = 'a cat, (bright eyes:1.2), <lora:example:0.8>, {colors}, tail';

beforeAll(async () => {
  const result = await build({
    entryPoints: ['tests/prompt_editor/support/BrowserProof.ts'],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
    loader: { '.txt': 'text' },
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const script = result.outputFiles[0]?.contents;
  if (!script) throw new Error('Missing prompt browser fixture');
  server = createServer((request, response) => {
    if (request.url === '/fixture.js') {
      response.writeHead(200, { 'Content-Type': 'application/javascript' });
      response.end(script);
    } else if (request.url?.startsWith('/proof/')) {
      response.writeHead(200, { 'Content-Type': 'image/svg+xml' });
      response.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="128"><rect width="512" height="128" fill="#3060a0"/><path d="M0 128L160 0 320 128Z" fill="#e09040"/></svg>',
      );
    } else {
      response.writeHead(200, { 'Content-Type': 'text/html' });
      response.end(
        '<!doctype html><html class="dark-theme" style="color-scheme:dark"><meta charset="utf-8"><style>body{margin:24px;background:#202020;color:#eee}textarea{width:700px}</style><script type="module" src="/fixture.js"></script></html>',
      );
    }
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const listening = server.address();
  if (!listening || typeof listening === 'string') throw new Error('Missing fixture address');
  address = `http://127.0.0.1:${String(listening.port)}`;
  browser = await chromium.launch({ headless: true });
}, 60_000);

afterAll(async () => {
  await closeBrowserFixture(browser, server);
}, 30_000);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s paints rich syntax before focus and retains every visible weight through native scrolling',
  async (renderer) => {
    const page = await browser.newPage();
    const longPrompt = Array.from(
      { length: 180 },
      (_, index) => `(row-${String(index)}:1.25), detailed portrait, blue eyes, flowing hair`,
    ).join('\n');
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, longPrompt }) => {
          window.promptBrowserProof.theme('photopro', true);
          window.promptBrowserProof.mount(renderer, 1, longPrompt);
        },
        { renderer, longPrompt },
      );
      await page.locator('main .cm-content').waitFor({ state: 'visible' });
      const observation = await page.evaluate(async () => {
        const editor = document.querySelector<HTMLElement>('main .cm-editor');
        const scroller = editor?.querySelector<HTMLElement>('.cm-scroller');
        if (!editor || !scroller) throw new Error('Missing native prompt');
        const failures: string[] = [];
        let inspected = 0;
        const frame = (): void => {
          if (editor.getAttribute('data-diagnostic-source') !== 'ready')
            failures.push('pending syntax');
          if (editor.contains(document.activeElement)) failures.push('unexpected focus');
          const viewport = scroller.getBoundingClientRect();
          for (const line of editor.querySelectorAll('.cm-line')) {
            const bounds = line.getBoundingClientRect();
            if (bounds.bottom <= viewport.top || bounds.top >= viewport.bottom) continue;
            if (!line.textContent?.includes('row-')) continue;
            inspected++;
            if (line.querySelectorAll('.sp-emphasis-weight').length !== 1)
              failures.push(line.textContent);
          }
        };
        frame();
        for (const fraction of [0.25, 0.5, 0.75, 1, 0.5, 0]) {
          scroller.scrollTop = fraction * (scroller.scrollHeight - scroller.clientHeight);
          await new Promise<void>((resolve) =>
            requestAnimationFrame(() => {
              frame();
              requestAnimationFrame(() => {
                frame();
                resolve();
              });
            }),
          );
        }
        return { failures, inspected, source: window.promptBrowserProof.read().source };
      });
      expect(observation.inspected).toBeGreaterThan(30);
      expect(observation.failures).toEqual([]);
      expect(observation.source).toBe(longPrompt);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s restores a retained PhotoPro prompt after workflow editor sessions reset',
  async (renderer) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, source }) => {
          window.promptBrowserProof.theme('photopro', true);
          window.promptBrowserProof.mount(renderer, 1, source);
        },
        { renderer, source },
      );
      await ready(page);
      for (let reset = 0; reset < 3; reset++) {
        await page.evaluate(() => {
          window.promptBrowserProof.reset();
          window.promptBrowserProof.refresh();
        });
        await expectPromptRestored(page);
      }
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+a');
      await page.keyboard.type('retained prompt');
      await page.waitForFunction(
        () => window.promptBrowserProof.read().source === 'retained prompt',
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s keeps the connected prompt when tab activation later prepares a detached graph projection',
  async (renderer) => {
    const page = await browser.newPage();
    const longPrompt = 'A retained prompt with enough content to wrap across many lines. '.repeat(
      30,
    );
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, longPrompt }) => {
          window.promptBrowserProof.theme('photopro', true);
          window.promptBrowserProof.mount(renderer, 1, longPrompt, true);
        },
        { renderer, longPrompt },
      );
      await page.locator('main [data-sc-prompt-editor] .cm-content').waitFor({ state: 'visible' });
      await page
        .locator('main [data-sc-prompt-editor]')
        .evaluate((element) => element.setAttribute('data-original-editor', ''));
      for (const connected of [false, true, false]) {
        await page.evaluate(
          (connected) => window.promptBrowserProof.addInactiveProjection(connected),
          connected,
        );
        expect(await page.locator('main [data-original-editor]').count()).toBe(1);
        expect(
          await page.locator('.late-native-graph-projection [data-sc-prompt-editor]').count(),
        ).toBe(0);
        expect(
          await page
            .locator('main textarea:not([data-sp-owned])')
            .evaluate((element: HTMLTextAreaElement) => element.hidden),
        ).toBe(true);
        expect(
          await page
            .locator('main [data-sc-prompt-editor]')
            .evaluate((element) => element.getBoundingClientRect().height),
        ).toBeGreaterThanOrEqual(128);
        expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(longPrompt);
        expect(
          await page
            .locator('main .cm-content')
            .evaluate((element) => (element as HTMLElement).isContentEditable),
        ).toBe(true);
        await page.evaluate(() => window.promptBrowserProof.removeDetachedProjection());
        expect(await page.locator('main [data-original-editor]').count()).toBe(1);
      }
      await page.evaluate(() => {
        window.promptBrowserProof.addInactiveProjection(false);
        window.promptBrowserProof.releaseVisibleProjection();
      });
      expect(
        await page.locator('.late-native-graph-projection [data-original-editor]').count(),
      ).toBe(1);
      expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

/** Require the rich presentation rather than the native textarea's two-line fallback. */
async function expectPromptRestored(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document.querySelector<HTMLElement>('[data-sc-prompt-editor]');
      return root && root.offsetHeight >= 128 && getComputedStyle(root).display !== 'none';
    },
    null,
    { timeout: 2000 },
  );
  expect(await page.locator('[data-sc-prompt-editor]').count()).toBe(1);
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s offers bundled Danbooru tags and preserves native acceptance history across renderer moves',
  async (renderer) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate((renderer) => window.promptBrowserProof.mount(renderer, 1, ''), renderer);
      await page.locator('.cm-content').focus();
      await page.keyboard.type('blue_ey');
      const option = page.getByRole('option', { name: /^blue eyes,/ });
      await option.waitFor({ timeout: 10_000 });
      await page.keyboard.press('Tab');
      await page.waitForFunction(() => window.promptBrowserProof.read().source === 'blue eyes, ');
      await page.keyboard.press('Control+z');
      await page.waitForFunction(() => window.promptBrowserProof.read().source === 'blue_ey');
      await page.keyboard.press('Control+y');
      await page.waitForFunction(() => window.promptBrowserProof.read().source === 'blue eyes, ');
      await page.evaluate(
        (nodesTwo: boolean) =>
          window.promptBrowserProof.switchRenderer(nodesTwo ? 'nodes-2' : 'nodes-1', 1),
        renderer === 'nodes-1',
      );
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+a');
      await page.keyboard.type('long_hai');
      await page.getByRole('option', { name: /^long hair,/ }).click();
      await page.waitForFunction(() => window.promptBrowserProof.read().source === 'long hair');
      const observed = await page.evaluate(() => window.promptBrowserProof.read());
      expect(observed.errors).toEqual([]);
      expect(observed.before).toBe(observed.after);
      expect(observed.escapedHistoryKeys).toBe(0);
    } finally {
      await page.close();
    }
  },
  60_000,
);

test.each([
  ['nodes-1', ' '],
  ['nodes-2', ' '],
  ['nodes-1', '\u00a0'],
  ['nodes-2', '\u00a0'],
  ['nodes-1', '\u202f'],
  ['nodes-2', '\u202f'],
] as const)(
  '%s preserves the preceding comma and %j separator when Tab completes the re:zero tag',
  async (renderer, separator) => {
    const page = await browser.newPage();
    const prefix =
      '(wind lift, floating hair, hat lift, skirt lift, hand on own hat, ribbon lift:3.00), fighting stance, holding down headwear, hand on headwear, (oversized brim:-1.00),' +
      separator;
    const completed = prefix + 're:zero kara hajimeru isekai seikatsu, ';
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, prefix }) => window.promptBrowserProof.mount(renderer, 1, prefix),
        { renderer, prefix },
      );
      await page.locator('[data-sc-prompt-editor] .cm-content').waitFor({ state: 'visible' });
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+End');
      await page.keyboard.type('re:');
      await page.getByRole('option', { name: /^re:zero kara hajimeru isekai seikatsu,/ }).waitFor();
      await page.keyboard.press('Tab');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        completed,
      );
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        prefix + 're:',
      );
      await page.keyboard.press('Control+y');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        completed,
      );
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(
  (
    [
      ['nodes-1', 'fluent'],
      ['nodes-2', 'fluent'],
      ['nodes-1', 'spectrum'],
      ['nodes-2', 'spectrum'],
      ['nodes-1', 'comfy'],
      ['nodes-2', 'comfy'],
    ] as const
  ).flatMap(([renderer, style]) =>
    [0.75, 1, 1.5].map((scale) => [renderer, style, scale] as const),
  ),
)(
  '%s %s at scale %s aligns the scrolling viewport with the field frame and retains native text spacing',
  async (renderer, style, scale) => {
    const page = await browser.newPage();
    const large = source + '\n' + 'blue ribbon, (bright eyes:1.20), long hair,\n'.repeat(70);
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, large, style, scale }) => {
          window.promptBrowserProof.mount(renderer, scale, large);
          window.promptBrowserProof.theme(style, true);
        },
        { renderer, large, style, scale },
      );
      await page.locator('.cm-content').waitFor({ state: 'visible' });
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+Home');
      await page.waitForFunction(
        () => {
          const field = document.querySelector('[data-sc-editor-field]');
          const slot = field?.shadowRoot?.querySelector('slot[name="editor"]') ?? field;
          const input = field?.shadowRoot?.querySelector('textarea') ?? field;
          const scroller = field?.querySelector('.cm-scroller');
          const content = field?.querySelector('.cm-content');
          if (!field || !slot || !input || !scroller || !content) return false;
          const border = field.shadowRoot?.querySelector('[part="root"]') ?? slot;
          if (!(border instanceof HTMLElement)) return false;
          const reference = input === field ? document.createElement('div') : null;
          if (reference) {
            reference.className = field.className;
            reference.style.cssText = 'position:absolute;visibility:hidden';
            field.after(reference);
          }
          const frame = border.getBoundingClientRect(),
            viewport = scroller.getBoundingClientRect(),
            style = getComputedStyle(border),
            provider = getComputedStyle(field),
            native = getComputedStyle(reference ?? input),
            text = getComputedStyle(content);
          const expected = {
            top: Number.parseFloat(native.paddingTop),
            right: Number.parseFloat(native.paddingRight),
            bottom: Number.parseFloat(native.paddingBottom),
            left: Number.parseFloat(native.paddingLeft),
          };
          reference?.remove();
          const scaleX = frame.width / border.offsetWidth;
          const scaleY = frame.height / border.offsetHeight;
          const block = Number.parseFloat(provider.getPropertyValue('--padding-block')) || 0;
          const inline = Number.parseFloat(provider.getPropertyValue('--padding-inline')) || 0;
          return (
            getComputedStyle(slot).overflow === 'visible' &&
            Math.abs(
              viewport.right - (frame.right - Number.parseFloat(style.borderRightWidth) * scaleX),
            ) < 1 &&
            Math.abs(
              viewport.left - (frame.left + Number.parseFloat(style.borderLeftWidth) * scaleX),
            ) < 1 &&
            Math.abs(
              viewport.top - (frame.top + Number.parseFloat(style.borderTopWidth) * scaleY),
            ) < 1 &&
            Math.abs(
              viewport.bottom -
                (frame.bottom - Number.parseFloat(style.borderBottomWidth) * scaleY),
            ) < 1 &&
            Number.parseFloat(text.paddingTop) === expected.top + block &&
            Number.parseFloat(text.paddingRight) === expected.right + inline &&
            Number.parseFloat(text.paddingBottom) === expected.bottom + block &&
            Number.parseFloat(text.paddingLeft) === expected.left + inline
          );
        },
        undefined,
        { timeout: 3000 },
      );
      await page.evaluate(() => {
        document.body.addEventListener(
          'wheel',
          (event) => {
            const capture =
              event.target instanceof Element
                ? event.target.closest('[data-capture-wheel="true"]')
                : null;
            if (capture?.contains(document.activeElement)) return;
            document.body.dataset.escapedPromptWheel = 'true';
            event.preventDefault();
          },
          { capture: true, passive: false },
        );
      });
      const scroller = page.locator('.cm-scroller');
      await page.waitForFunction(
        () =>
          document.querySelector('.cm-editor')?.getAttribute('data-diagnostic-source') === 'ready',
      );
      await scroller.hover();
      await page.mouse.wheel(0, 240);
      await page.waitForFunction(
        () => (document.querySelector('.cm-scroller')?.scrollTop ?? 0) > 0,
      );
      expect(await page.evaluate(() => document.body.dataset.escapedPromptWheel)).toBeUndefined();
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+End');
      await page.keyboard.type('q');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large + 'q',
      );
      await page.waitForFunction(
        () =>
          document.querySelector('.cm-editor')?.getAttribute('data-diagnostic-source') === 'ready',
      );
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large,
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    (['fluent', 'spectrum', 'comfy'] as const).map((style) => [renderer, style] as const),
  ),
)(
  '%s %s advertises caret placement across text, padding and blank space while retaining Alt drag cursors',
  async (renderer, style) => {
    const page = await browser.newPage();
    const prompt = 'alpha, (bright eyes:1.20), {colors}, tail\nsecond line';
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, style, prompt }) => {
          window.promptBrowserProof.mount(renderer, 1, prompt);
          window.promptBrowserProof.theme(style, true);
        },
        { renderer, style, prompt },
      );
      await page.locator('.cm-content').waitFor();
      await page.locator('[data-sc-prompt-editor]').evaluate((frame) => {
        if (!(frame instanceof HTMLElement)) throw new Error('Missing prompt field');
        frame.style.cursor = 'grab';
      });
      for (const zone of ['text', 'padding', 'blank'] as const) {
        await page.evaluate((source) => window.promptBrowserProof.replace(source), prompt);
        await page.locator('.cm-editor[data-diagnostic-source="ready"]').waitFor();
        const point = await page.locator('.cm-content').evaluate((content, zone) => {
          const scroller = content.closest('.cm-scroller');
          if (!scroller) throw new Error('Missing native scroll viewport');
          const frame = scroller.getBoundingClientRect();
          if (zone === 'padding') return { x: frame.left + 1, y: frame.top + 1 };
          if (zone === 'blank') return { x: frame.left + frame.width / 2, y: frame.bottom - 8 };
          const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
          for (let node = walker.nextNode(); node; node = walker.nextNode()) {
            if (!node.nodeValue?.startsWith('alpha')) continue;
            const range = document.createRange();
            range.setStart(node, 2);
            range.collapse(true);
            const rect = range.getBoundingClientRect();
            return { x: rect.left, y: rect.top + rect.height / 2 };
          }
          throw new Error('Missing visible source text');
        }, zone);
        await page.mouse.move(point.x, point.y);
        const cursor = await page.evaluate(({ x, y }) => {
          let hit = document.elementFromPoint(x, y);
          while (hit?.shadowRoot) {
            const child = hit.shadowRoot.elementFromPoint(x, y);
            if (!child || child === hit) break;
            hit = child;
          }
          return hit ? getComputedStyle(hit).cursor : null;
        }, point);
        expect(cursor).toBe('text');
        await page.mouse.click(point.x, point.y);
        await page.keyboard.type('X');
        const expected =
          zone === 'text'
            ? 'alX' + prompt.slice(2)
            : zone === 'padding'
              ? 'X' + prompt
              : prompt + 'X';
        await page.waitForFunction(
          (source) => window.promptBrowserProof.read().source === source,
          expected,
          { timeout: 3000 },
        );
      }
      await page.evaluate(() => window.promptBrowserProof.replace(''));
      await page.locator('.cm-scroller').click();
      await page.keyboard.type('empty field');
      await page.waitForFunction(() => window.promptBrowserProof.read().source === 'empty field');
      await page.evaluate((source) => window.promptBrowserProof.replace(source), prompt);
      await page.locator('.cm-editor[data-diagnostic-source="ready"]').waitFor();
      const chip = page.locator('.sp-reorder-chip').first();
      await chip.hover();
      expect(await chip.evaluate((element) => getComputedStyle(element).cursor)).toBe('text');
      await page.locator('.cm-content').focus();
      await page.keyboard.down('Alt');
      await page.waitForFunction(
        () => getComputedStyle(document.querySelector('.sp-reorder-chip')!).cursor === 'grab',
      );
      await page.mouse.down();
      await page.waitForFunction(
        () => getComputedStyle(document.querySelector('.sp-reorder-chip')!).cursor === 'grabbing',
      );
      await page.mouse.up();
      await page.keyboard.up('Alt');
      await page.waitForFunction(
        () => getComputedStyle(document.querySelector('.sp-reorder-chip')!).cursor === 'text',
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(prompt);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    (['fluent', 'spectrum', 'comfy'] as const).map((style) => [renderer, style] as const),
  ),
)(
  '%s %s uses the browser caret and selection without a separate animated cursor',
  async (renderer, style) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, style, source }) => {
          window.promptBrowserProof.mount(renderer, 1, source);
          window.promptBrowserProof.theme(style, true);
        },
        { renderer, style, source },
      );
      await ready(page);
      await page.locator('.cm-content').click();
      await page.keyboard.press('Control+End');
      await page.keyboard.type('typingproof');
      const painting = await page.locator('.cm-content').evaluate((content) => {
        const selection = getSelection();
        const editor = content.closest('.cm-editor');
        return {
          caret: getComputedStyle(content).caretColor,
          foreground: getComputedStyle(content).color,
          leafCarets: Array.from(content.querySelectorAll('.cm-line, .sp-reorder-chip')).map(
            (node) => getComputedStyle(node).caretColor,
          ),
          overlays: editor?.querySelectorAll('.cm-cursorLayer,.cm-selectionLayer').length,
          inside: !!selection?.focusNode && content.contains(selection.focusNode),
          collapsed: selection?.isCollapsed,
        };
      });
      expect(painting.caret).toBe(painting.foreground);
      expect(painting.leafCarets.every((color) => color === painting.caret)).toBe(true);
      expect(painting.overlays).toBe(0);
      expect(painting.inside).toBe(true);
      expect(painting.collapsed).toBe(true);
      await page.keyboard.press('Control+a');
      expect(await page.evaluate(() => getSelection()?.toString())).toContain('typing');
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (source) => window.promptBrowserProof.read().source === source,
        source,
        { timeout: 3000 },
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test('uses provider field controls while retaining the same native document across all visual styles', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    await page.locator('.cm-content').focus();
    await page.keyboard.press('Control+End');
    await page.keyboard.type('q');
    const content = await page.locator('.cm-content').elementHandle();
    for (const style of ['fluent', 'spectrum', 'comfy', 'fluent'] as const) {
      await page.evaluate((style) => window.promptBrowserProof.theme(style, true), style);
      const field = page.locator(`[data-sc-editor-field="${style}"]`);
      await field.waitFor({ state: 'visible' });
      expect(await field.evaluate((node) => node.tagName)).toBe(
        style === 'fluent'
          ? 'FLUENT-TEXTAREA'
          : style === 'spectrum'
            ? 'SC-SPECTRUM-EDITOR-FIELD'
            : 'DIV',
      );
      expect(
        await page.locator('.cm-content').evaluate((node, retained) => node === retained, content),
      ).toBe(true);
      expect(
        await page.locator('.cm-content').evaluate((node) => node === document.activeElement),
      ).toBe(true);
      expect(await field.evaluate((node) => node.matches(':focus-within'))).toBe(true);
      if (style === 'spectrum')
        expect(await field.evaluate((node) => node.hasAttribute('focused'))).toBe(true);
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source + 'q');
    }
    await page.keyboard.press('Control+z');
    await page.waitForFunction(
      (source) => window.promptBrowserProof.read().source === source,
      source,
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 60_000);

test('native typing retains unrelated wrapping DOM while semantic preparation is pending', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    const retained = await page.locator('.sp-prompt-keep').first().elementHandle();
    if (!retained) throw new Error('Missing initial wrapping mark');
    await page.locator('.cm-content [data-reorder-chip="0"]').click();
    await page.keyboard.press('Control+End');
    await page.keyboard.press('q');
    await page.waitForFunction(
      (source) => window.promptBrowserProof.read().source === source + 'q',
      source,
    );
    expect(await retained.evaluate((element) => element.isConnected)).toBe(true);
    await page.keyboard.press('Control+z');
    await page.waitForFunction(
      (source) => window.promptBrowserProof.read().source === source,
      source,
    );
  } finally {
    await page.close();
  }
});

test.each([
  ['nodes-1', 1],
  ['nodes-1', 0.85],
  ['nodes-2', 1],
  ['nodes-2', 0.85],
] as const)(
  '%s at scale %s bounds large prompt paint and preserves visible burst typing, selection and native undo',
  async (renderer, scale) => {
    const page = await browser.newPage();
    const large =
      'best quality, (bright blue eyes:1.20), {colors}, long-sleeved, tail, '.repeat(1600) +
      source +
      '\nend typing target ';
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, scale, large }) => window.promptBrowserProof.mount(renderer, scale, large),
        { renderer, scale, large },
      );
      await page.locator('.cm-content').waitFor();
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+End');
      await page.waitForFunction(() => {
        const selection = getSelection();
        return (
          selection?.isCollapsed &&
          !!selection.focusNode &&
          document.querySelector('.cm-content')?.contains(selection.focusNode)
        );
      });
      await page.waitForFunction(
        () => document.querySelector('.cm-content')!.querySelectorAll('*').length < 3500,
      );
      await page.locator('.cm-editor[data-diagnostic-source="ready"]').waitFor();
      await page.locator('.cm-content').evaluate((content) => {
        const samples: boolean[] = [];
        const range = document.createRange();
        Reflect.set(content, 'typingVisibilityProof', samples);
        content.addEventListener('keydown', (event) => {
          if (
            !(event instanceof KeyboardEvent) ||
            event.key.length !== 1 ||
            event.ctrlKey ||
            event.metaKey ||
            event.altKey
          )
            return;
          requestAnimationFrame(() => {
            const selection = getSelection(),
              scroller = content.closest('.cm-scroller');
            if (!selection?.focusNode || selection.focusOffset < 1 || !scroller) {
              samples.push(false);
              return;
            }
            range.setStart(selection.focusNode, selection.focusOffset - 1);
            range.setEnd(selection.focusNode, selection.focusOffset);
            const rect = range.getBoundingClientRect(),
              viewport = scroller.getBoundingClientRect();
            samples.push(
              rect.width > 0 &&
                rect.bottom > viewport.top &&
                rect.top < viewport.bottom &&
                rect.right > viewport.left &&
                rect.left < viewport.right,
            );
            range.setStart(content, 0);
            range.collapse(true);
          });
        });
      });
      const typed = 'qwertyuiopasdfghjklzxcvbnm'.repeat(4);
      await page.keyboard.type(typed);
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large + typed,
      );
      const visible = await page.locator('.cm-content').evaluate(async (content) => {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        const samples: unknown = Reflect.get(content, 'typingVisibilityProof');
        if (!Array.isArray(samples) || !samples.every((value) => typeof value === 'boolean'))
          throw new Error('Missing native typing visibility proof');
        return samples.map((value: unknown) => value === true);
      });
      expect(visible).toHaveLength(typed.length);
      expect(visible.every(Boolean)).toBe(true);
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large,
      );
      await page.keyboard.press('Control+y');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large + typed,
      );
      await page.keyboard.press('Control+Home');
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('q');
      await page.keyboard.down('Alt');
      await page
        .locator('.cm-editor.sp-reorder-preview,.cm-content .sp-reorder-preview')
        .first()
        .waitFor();
      await page.keyboard.up('Alt');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        'bq' + large.slice(1) + typed,
      );
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        large + typed,
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s shows the caret and accent Alt bubbles without blank-line paint or a Fluent focus rectangle',
  async (renderer) => {
    const page = await browser.newPage();
    const multiline = source + '\n\nblue ribbon, pink hair';
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, multiline }) => window.promptBrowserProof.mount(renderer, 1, multiline),
        { renderer, multiline },
      );
      await ready(page);
      for (const dark of [true, false]) {
        await page.evaluate((dark) => window.promptBrowserProof.theme('fluent', dark), dark);
        await page.locator('.cm-content').click();
        await page.keyboard.press('Control+End');
        const paint = await page.locator('[data-sc-prompt-editor]').evaluate((root) => {
          const provider = root.querySelector('fluent-textarea');
          const chrome = provider?.shadowRoot?.querySelector('[part="root"]');
          const content = root.querySelector('.cm-content');
          if (!chrome || !content) throw new Error('Missing official Fluent textarea');
          const field = getComputedStyle(chrome);
          const selection = getSelection();
          return {
            foreground: getComputedStyle(content).color,
            caret: getComputedStyle(content).caretColor,
            nativeSelection:
              selection?.isCollapsed &&
              !!selection.focusNode &&
              content.contains(selection.focusNode),
            caretHeight: selection?.rangeCount
              ? selection.getRangeAt(0).getBoundingClientRect().height
              : 0,
            top: field.borderTopColor,
            right: field.borderRightColor,
            accent: getComputedStyle(chrome, '::after').borderBottomColor,
            outline: field.outlineStyle,
            shadow: field.boxShadow,
          };
        });
        expect(paint.caret).toBe(paint.foreground);
        expect(paint.nativeSelection).toBe(true);
        expect(paint.caretHeight).toBeGreaterThan(8);
        expect(paint.top).not.toBe(paint.accent);
        expect(paint.right).not.toBe(paint.accent);
        expect(paint.outline).toBe('none');
        expect(paint.shadow).toBe('none');
        await page.keyboard.down('Alt');
        await page
          .locator('.cm-editor.sp-reorder-preview,.cm-content .sp-reorder-preview')
          .first()
          .waitFor();
        const bubbles = await page.locator('.cm-content [data-reorder-chip]').evaluateAll((nodes) =>
          nodes.map((node) => ({
            outline: getComputedStyle(node).outlineColor,
            style: getComputedStyle(node).outlineStyle,
            fill: getComputedStyle(node).backgroundColor,
          })),
        );
        expect(bubbles.length).toBeGreaterThan(4);
        expect(
          bubbles.every((bubble) => bubble.outline === paint.accent && bubble.style === 'solid'),
        ).toBe(true);
        const blanks = await page.locator('.cm-content .sp-reorder-empty').evaluateAll((nodes) =>
          nodes.map((node) => ({
            border: getComputedStyle(node).borderBottomStyle,
            outline: getComputedStyle(node).outlineStyle,
            background: getComputedStyle(node).backgroundColor,
          })),
        );
        expect(blanks.length).toBeGreaterThan(0);
        expect(
          blanks.every(
            (blank) =>
              blank.border === 'none' &&
              blank.outline === 'none' &&
              blank.background === 'rgba(0, 0, 0, 0)',
          ),
        ).toBe(true);
        await page.keyboard.up('Alt');
        await page.keyboard.insertText('!');
        await page.waitForFunction(
          (value) => window.promptBrowserProof.read().source === value + '!',
          multiline,
        );
        await page.keyboard.press('Control+z');
        await page.waitForFunction(
          (value) => window.promptBrowserProof.read().source === value,
          multiline,
        );
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);

test.each(['fluent', 'spectrum', 'comfy'] as const)(
  '%s provider owns field focus in both palettes without recreating the editor',
  async (style) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate((value) => window.promptBrowserProof.mount('nodes-2', 1, value), source);
      await ready(page);
      for (const dark of [true, false]) {
        await page.evaluate(({ style, dark }) => window.promptBrowserProof.theme(style, dark), {
          style,
          dark,
        });
        await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
        await page.locator('[data-sc-prompt-editor]').evaluate(async (root) => {
          await Promise.allSettled(
            root
              .getAnimations({ subtree: true })
              .filter((animation) => animation.effect?.getTiming().iterations !== Infinity)
              .map((animation) => animation.finished),
          );
        });
        if (style === 'fluent')
          await page.waitForFunction(() => {
            const chrome = document
              .querySelector('fluent-textarea')
              ?.shadowRoot?.querySelector('[part="root"]');
            return (
              chrome && getComputedStyle(chrome, '::after').transform === 'matrix(1, 0, 0, 1, 0, 0)'
            );
          });
        const paint = await page.locator('[data-sc-prompt-editor]').evaluate((root) => {
          const content = root.querySelector('.cm-content');
          const editor = root.querySelector('.cm-editor');
          if (!content || !editor) throw new Error('Missing native editor');
          const provider = root.querySelector('[data-sc-editor-field]');
          if (!provider) throw new Error('Missing provider field');
          const chrome = provider.shadowRoot?.querySelector('[part="root"],#textfield') ?? provider;
          const field = getComputedStyle(chrome),
            underline = getComputedStyle(chrome, '::after');
          return {
            skin: root.getAttribute('data-sc-prompt-style'),
            provider: provider.tagName,
            spectrumFocused: provider.hasAttribute('focused'),
            underline: underline.display,
            height: underline.borderBottomWidth,
            color: underline.borderBottomColor,
            transform: underline.transform,
            caret: getComputedStyle(content).caretColor,
            border: field.borderBottomColor,
            top: field.borderTopColor,
            right: field.borderRightColor,
            outerBorder: getComputedStyle(root).borderTopWidth,
            outerIndicator: getComputedStyle(root, '::after').content,
            hostFrame: root.parentElement ? getComputedStyle(root.parentElement).boxShadow : null,
            shadow: getComputedStyle(editor).boxShadow,
            nativeOutline: getComputedStyle(editor).outlineStyle,
            focused: root.contains(document.activeElement),
          };
        });
        expect(paint.skin).toBe(style);
        expect(paint.focused).toBe(true);
        expect(paint.shadow).toBe('none');
        expect(paint.nativeOutline).toBe('none');
        expect(paint.outerBorder).toBe('0px');
        expect(paint.outerIndicator).toBe('none');
        if (style === 'fluent') {
          expect(paint.provider).toBe('FLUENT-TEXTAREA');
          expect(paint.underline).toBe('block');
          expect(paint.height).toBe('2px');
          expect(paint.transform).toBe('matrix(1, 0, 0, 1, 0, 0)');
          expect(paint.top).not.toBe(paint.color);
          expect(paint.right).not.toBe(paint.color);
          expect(paint.hostFrame).toBe('none');
        } else if (style === 'spectrum') {
          expect(paint.provider).toBe('SC-SPECTRUM-EDITOR-FIELD');
          expect(paint.spectrumFocused).toBe(true);
          expect(paint.hostFrame).toBe('none');
        } else {
          expect(paint.provider).toBe('DIV');
          expect(paint.hostFrame).not.toBe('none');
        }
        expect(paint.border).not.toBe('rgb(41, 156, 255)');
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
      expect(await page.locator('[data-sc-prompt-editor]').count()).toBe(1);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s PhotoPro prompt editor follows live palette changes without remounting',
  async (renderer) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, value }) => window.promptBrowserProof.mount(renderer, 1, value),
        {
          renderer,
          value: source,
        },
      );
      await ready(page);
      await page.evaluate(() => window.promptBrowserProof.theme('photopro', true));
      const editor = page.locator('[data-sc-prompt-editor] .cm-editor');
      await editor.evaluate((element) => {
        if (!(element instanceof HTMLElement)) throw new Error('Missing native editor element.');
        element.dataset.paletteProbe = 'original';
      });
      for (const [palette, expectedScheme] of [
        ['black', 'dark'],
        ['light-gray', 'light'],
        ['medium-gray', 'dark'],
      ] as const) {
        await page.evaluate((value) => {
          document.documentElement.dataset.scPhotoproPalette = value;
        }, palette);
        expect(
          await page
            .locator('[data-sc-prompt-editor]')
            .evaluate((root) => getComputedStyle(root).colorScheme),
        ).toBe(expectedScheme);
        const contentPadding = await page
          .locator('[data-sc-prompt-editor] .cm-content')
          .evaluate((content) => {
            const style = getComputedStyle(content);
            return {
              left: Number.parseFloat(style.paddingLeft),
              right: Number.parseFloat(style.paddingRight),
            };
          });
        expect(contentPadding.left).toBeGreaterThanOrEqual(6);
        expect(contentPadding.right).toBe(6);
        expect(await editor.getAttribute('data-palette-probe')).toBe('original');
        expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
      }
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s PhotoPro prompt editor wraps long text without a horizontal scrollbar',
  async (renderer) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(() => window.promptBrowserProof.theme('photopro', true));
      const longPrompt = 'unbroken_prompt_token'.repeat(12);
      await page.evaluate(
        ({ renderer, value }) => window.promptBrowserProof.mount(renderer, 1, value),
        { renderer, value: longPrompt },
      );
      await page.locator('[data-sc-prompt-editor] .cm-content').waitFor({ state: 'visible' });
      const overflow = await page.locator('[data-sc-prompt-editor]').evaluate((root) => {
        const field = root.querySelector<HTMLElement>('.sc-photopro-editor-field');
        const scroller = root.querySelector<HTMLElement>('.cm-scroller');
        const line = root.querySelector<HTMLElement>('.cm-line');
        if (!field || !scroller || !line) throw new Error('Missing prompt scroll surfaces.');
        return {
          field: getComputedStyle(field).overflowX,
          scroller: getComputedStyle(scroller).overflowX,
          lineHeight: line.getBoundingClientRect().height,
        };
      });
      expect(overflow.field).toBe('hidden');
      expect(overflow.scroller).toBe('hidden');
      expect(overflow.lineHeight).toBeGreaterThan(30);
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(longPrompt);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test('publish the minimum prompt allocation before native renderer sizing can feed back', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.evaluate((value) => window.promptBrowserProof.mount('nodes-2', 1, value), source);
    await ready(page);
    await expect(page.evaluate(() => window.promptBrowserProof.read().allocation)).resolves.toBe(
      148,
    );
    await page.evaluate(() => window.promptBrowserProof.switchRenderer('nodes-1', 1));
    await ready(page);
    await expect(page.evaluate(() => window.promptBrowserProof.read().allocation)).resolves.toBe(
      148,
    );
  } finally {
    await page.close();
  }
});

test('an empty prompt shows an insertion caret before typing and after undo', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate(() => {
      window.promptBrowserProof.theme('fluent', true);
      window.promptBrowserProof.mount('nodes-2', 1, '');
    });
    await page.locator('.cm-content').click();
    await page.waitForFunction(() => {
      const root = document.querySelector('[data-sc-prompt-editor]'),
        content = root?.querySelector('.cm-content'),
        selection = getSelection();
      return (
        content &&
        selection?.isCollapsed &&
        !!selection.focusNode &&
        content.contains(selection.focusNode) &&
        getComputedStyle(content).caretColor === getComputedStyle(content).color
      );
    });
    await page.keyboard.type('a');
    await page.waitForFunction(() => window.promptBrowserProof.read().source === 'a');
    await page.keyboard.press('Control+z');
    await page.waitForFunction(() => window.promptBrowserProof.read().source === '');
    expect(
      await page.locator('.cm-content').evaluate((content) => {
        const selection = getSelection();
        return (
          selection?.isCollapsed &&
          !!selection.focusNode &&
          content.contains(selection.focusNode) &&
          getComputedStyle(content).caretColor === getComputedStyle(content).color
        );
      }),
    ).toBe(true);
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 30_000);

test('wrapping keeps short tags whole and attaches decorations to the edge words of long segments', async () => {
  const page = await browser.newPage();
  const browserErrors: string[] = [];
  page.on('pageerror', (error) => browserErrors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') browserErrors.push(message.text());
  });
  const prompt =
    'alpha, best quality, (holding large wooden staff:1.60), one two long-sleeved 三色毛 hair extra, tail';
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.addStyleTag({ content: '.lg-node{width:var(--proof-card-width,240px)!important}' });
    await page.evaluate((prompt) => window.promptBrowserProof.mount('nodes-2', 1, prompt), prompt);
    await page.locator('.sp-emphasis-suffix').waitFor();
    const observations = await page.evaluate(async () => {
      const result: {
        width: number;
        tagLines: number;
        leadingDelta: number;
        trailingDelta: number;
        joinedHeight: number;
        unicodeHeight: number;
      }[] = [];
      const content = document.querySelector('.cm-content');
      if (!content) throw new Error('Missing prompt content');
      /** Measure actual glyphs independently of wrapping decoration implementation. */
      const word = (text: string): DOMRect => {
        const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
          const offset = node.textContent?.indexOf(text) ?? -1;
          if (offset < 0) continue;
          const range = document.createRange();
          range.setStart(node, offset);
          range.setEnd(node, offset + text.length);
          return range.getBoundingClientRect();
        }
        throw new Error('Missing word ' + text);
      };
      for (const width of [
        112, 125, 140, 155, 170, 185, 200, 215, 230, 245, 260, 275, 290, 305, 320,
      ]) {
        document.documentElement.style.setProperty('--proof-card-width', `${String(width)}px`);
        for (let frame = 0; frame < 5; frame++)
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        const best = word('best'),
          quality = word('quality'),
          holding = word('holding'),
          staff = word('staff');
        const prefix = content.querySelector('.sp-emphasis-prefix')?.getBoundingClientRect(),
          suffix = content.querySelector('.sp-emphasis-suffix')?.getBoundingClientRect();
        if (!prefix || !suffix) throw new Error('Missing emphasis decorations');
        result.push({
          width,
          tagLines: Math.abs(best.top - quality.top),
          leadingDelta: Math.abs((holding.top + holding.bottom - prefix.top - prefix.bottom) / 2),
          trailingDelta: Math.abs((staff.top + staff.bottom - suffix.top - suffix.bottom) / 2),
          joinedHeight: word('long-sleeved').height,
          unicodeHeight: word('三色毛').height,
        });
      }
      return result;
    });
    expect(observations.filter((observation) => observation.tagLines > 2)).toEqual([]);
    expect(observations.filter((observation) => observation.leadingDelta > 8)).toEqual([]);
    expect(observations.filter((observation) => observation.trailingDelta > 8)).toEqual([]);
    expect(
      observations.filter(
        (observation) => observation.joinedHeight > 22 || observation.unicodeHeight > 22,
      ),
    ).toEqual([]);
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(prompt);
    expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    expect(browserErrors).toEqual([]);
  } finally {
    await page.close();
  }
}, 30_000);

test('oversized tags wrap within the field and raw mode retires all grouping without editing source', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.addStyleTag({ content: '.lg-node{width:112px!important}' });
    const oversized = 'alpha, extraordinarilylongwordthatcannotfit fresh line, tail';
    await page.evaluate((value) => window.promptBrowserProof.mount('nodes-2', 1, value), oversized);
    await page.locator('.sp-prompt-keep').first().waitFor();
    await page.waitForFunction(() => {
      const content = document.querySelector<HTMLElement>('.cm-content');
      const scroller = document.querySelector<HTMLElement>('.cm-scroller');
      const oversizedKept = Array.from(document.querySelectorAll('.sp-prompt-keep')).some((node) =>
        node.textContent?.includes('extraordinarily'),
      );
      return (
        content &&
        scroller &&
        !oversizedKept &&
        content.scrollWidth <= content.clientWidth + 1 &&
        scroller.scrollWidth <= scroller.clientWidth + 1
      );
    });
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(oversized);
    const plain = 'alpha, best quality, tail';
    await page.evaluate((value) => window.promptBrowserProof.replace(value), plain);
    await page.locator('.sp-prompt-keep').first().waitFor();
    await page.locator('.cm-content').click({ button: 'right' });
    await page
      .locator('[data-sc-prompt-menu]')
      .getByRole('menuitemcheckbox', { name: /rich|decoration/i })
      .click();
    await page.waitForFunction(() => document.querySelectorAll('.sp-prompt-keep').length === 0);
    expect(
      await page.locator('.cm-content [data-reorder-chip="1"]').evaluate((chip) => {
        const text = chip.firstChild;
        if (!text || text.nodeType !== Node.TEXT_NODE) throw new Error('Missing literal raw tag');
        const best = document.createRange(),
          quality = document.createRange();
        best.setStart(text, 0);
        best.setEnd(text, 4);
        quality.setStart(text, 5);
        quality.setEnd(text, 12);
        return Math.abs(best.getBoundingClientRect().top - quality.getBoundingClientRect().top);
      }),
    ).toBeGreaterThan(10);
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(plain);
    expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
  } finally {
    await page.close();
  }
}, 30_000);

test('caret tracks collapsed selection, typing and scrolled rich text across renderer changes', async () => {
  const page = await browser.newPage();
  const longSource =
    source + '\n' + 'best quality, (bright eyes:1.2), tail\n'.repeat(24) + 'caret target';
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((value) => {
      window.promptBrowserProof.theme('fluent', true);
      window.promptBrowserProof.mount('nodes-1', 0.85, value);
    }, longSource);
    for (const renderer of ['nodes-1', 'nodes-2'] as const) {
      if (renderer === 'nodes-2')
        await page.evaluate(() => window.promptBrowserProof.switchRenderer('nodes-2', 0.85));
      await ready(page);
      await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
      await page.keyboard.press('Control+End');
      await page.keyboard.press('Control+a');
      await page.keyboard.press('ArrowRight');
      await page.keyboard.insertText('!');
      await page.waitForFunction(
        (value) => window.promptBrowserProof.read().source === value + '!',
        longSource,
      );
      await page.waitForFunction(() => {
        const selection = getSelection(),
          content = document.querySelector('.cm-content'),
          scroller = document.querySelector('.cm-scroller');
        if (
          !selection?.isCollapsed ||
          !selection.focusNode ||
          selection.focusOffset < 1 ||
          !content?.contains(selection.focusNode) ||
          !scroller
        )
          return false;
        const range = document.createRange();
        range.setStart(selection.focusNode, selection.focusOffset - 1);
        range.setEnd(selection.focusNode, selection.focusOffset);
        const glyph = range.getBoundingClientRect(),
          caret = selection.getRangeAt(0).getBoundingClientRect(),
          viewport = scroller.getBoundingClientRect();
        return (
          scroller.scrollTop > 0 &&
          Math.abs(glyph.right - caret.left) < 2 &&
          Math.abs(glyph.top - caret.top) < 3 &&
          caret.top >= viewport.top &&
          caret.bottom <= viewport.bottom
        );
      });
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (value) => window.promptBrowserProof.read().source === value,
        longSource,
      );
    }
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 60_000);

/** Wait on semantic rendered output and actual decoded media rather than an arbitrary settling delay. */
async function ready(page: Page): Promise<void> {
  await page.locator('[data-sc-prompt-editor] .cm-content').waitFor({ state: 'visible' });
  await page.locator('.sp-lora-chip[data-media="ready"]').first().waitFor({ state: 'visible' });
}

test.each([
  ['nodes-1', 1],
  ['nodes-1', 0.85],
  ['nodes-2', 1],
  ['nodes-2', 0.85],
] as const)(
  '%s at scale %s preserves rich source, banner media, native history and renderer switching',
  async (renderer, scale) => {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, scale, source }) => window.promptBrowserProof.mount(renderer, scale, source),
        { renderer, scale, source },
      );
      await ready(page);
      expect(await page.locator('.sp-lora-title').textContent()).toBe('Example <b>model</b>');
      expect(await page.locator('.sp-lora-title b').count()).toBe(0);
      const pixel = await page.locator('.sp-lora-media').evaluate((element) => {
        if (!(element instanceof HTMLCanvasElement)) throw new Error('Missing decoded LoRA canvas');
        const canvas = element;
        return Array.from(
          canvas
            .getContext('2d')
            ?.getImageData(Math.floor(canvas.width * 0.9), Math.floor(canvas.height * 0.5), 1, 1)
            .data ?? [],
        );
      });
      expect(pixel).toEqual([48, 96, 160, 255]);
      expect(await page.locator('.sp-wildcard-prefix').count()).toBe(1);
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
      const content = page.locator('[data-sc-prompt-editor] .cm-content');
      await content.locator('[data-reorder-chip="0"]').click();
      await page.keyboard.press('Control+End');
      await page.keyboard.insertText(', new detail');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        source + ', new detail',
      );
      await page.keyboard.press('Control+z');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        source,
      );
      const transition: { opposite: 'nodes-1' | 'nodes-2'; scale: number } = {
        opposite: renderer === 'nodes-1' ? 'nodes-2' : 'nodes-1',
        scale,
      };
      await page.evaluate(
        ({ opposite, scale }) => window.promptBrowserProof.switchRenderer(opposite, scale),
        transition,
      );
      await ready(page);
      await page.locator('[data-sc-prompt-editor] .cm-content [data-reorder-chip="0"]').click();
      await page.keyboard.press('Control+y');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        source + ', new detail',
      );
      const state = await page.evaluate(() => window.promptBrowserProof.read());
      expect(state.before).toBe(state.after);
      expect(state.callbacks).toBeGreaterThan(0);
      expect(state.escapedHistoryKeys).toBe(0);
      expect(state.errors).toEqual([]);
      expect(errors).toEqual([]);
    } catch (cause) {
      await page.screenshot({
        path: `D:/SugarCubes/prompt-editor/failure-${renderer}-${String(scale)}.png`,
      });
      throw cause;
    } finally {
      await page.close();
    }
  },
  60_000,
);

/** Choose a painted fragment instead of the empty union between wrapped inline boxes. */
async function chipPoint(
  page: Page,
  chip: number,
  trailing: boolean,
): Promise<{ x: number; y: number }> {
  return page
    .locator(`.cm-content [data-reorder-chip="${String(chip)}"]`)
    .first()
    .evaluate((element, trailing) => {
      const lora = element.querySelector('.sp-lora-chip');
      const painted = lora?.closest('[data-reorder-chip]') === element ? lora : element;
      const range = element.ownerDocument.createRange();
      range.selectNodeContents(painted);
      const fragments = Array.from(range.getClientRects()).filter(
        (rect) => rect.width > 0 && rect.height > 0,
      );
      const rect = trailing ? fragments.at(-1) : fragments[0];
      if (!rect) throw new Error('Missing painted chip fragment');
      return {
        x: rect.left + (trailing ? rect.width - 1 : rect.width * 0.45),
        y: rect.top + rect.height / 2,
      };
    }, trailing);
}

/** Drive the real Alt gesture through native carrier geometry and observe its painted proxy. */
async function dragChip(
  page: Page,
  chip: number,
  destination: number,
  appendToRow = false,
): Promise<void> {
  await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
  await page.keyboard.down('Alt');
  await page
    .locator('.cm-editor.sp-reorder-preview, .cm-content .sp-reorder-preview')
    .first()
    .waitFor({ state: 'visible' });
  const start = await chipPoint(page, chip, false);
  const end = await chipPoint(page, destination, true);
  if (appendToRow) {
    const editor = await page.locator('[data-sc-prompt-editor]').boundingBox();
    if (!editor) throw new Error('Missing editor drop boundary');
    end.x = editor.x + editor.width - 12;
  }
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x + 12, start.y);
  await page.locator('.sp-reorder-proxy').waitFor({ state: 'visible' });
  await page.mouse.move(end.x, end.y, { steps: 12 });
  await page.mouse.up();
  await page.keyboard.up('Alt');
  await page.locator('.sp-reorder-proxy').waitFor({ state: 'hidden' });
}

test.each([
  ['nodes-1', 1],
  ['nodes-1', 0.85],
  ['nodes-2', 1],
  ['nodes-2', 0.85],
] as const)(
  '%s at scale %s drags decorated emphasis, LoRA and wildcard chips with native undo',
  async (renderer, scale) => {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    const parts = ['a cat', '(bright eyes:1.2)', '<lora:example:0.8>', '{colors}', 'tail'];
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      for (const chip of [1, 2, 3]) {
        await page.evaluate(
          ({ renderer, scale, source }) => window.promptBrowserProof.mount(renderer, scale, source),
          { renderer, scale, source },
        );
        await ready(page);
        await dragChip(page, chip, 4, true);
        const expected = [...parts.filter((_, index) => index !== chip), parts[chip]].join(', ');
        await page.waitForFunction(
          (expected) => window.promptBrowserProof.read().source === expected,
          expected,
        );
        expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(1);
        await page.keyboard.press('Control+z');
        await page.waitForFunction(
          (expected) => window.promptBrowserProof.read().source === expected,
          source,
        );
        expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
      }
    } catch (cause) {
      await page.screenshot({
        path: `D:/SugarCubes/prompt-editor/drag-failure-${renderer}-${String(scale)}.png`,
      });
      throw cause;
    } finally {
      await page.close();
    }
  },
  60_000,
);

test('fractional-scale heavy rich prompts retain decorated chip pickup, banner pixels, drop and undo', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const group = ['a cat', '(bright eyes:1.2)', '<lora:example:0.8>', '{colors}', 'tail'];
  const parts = Array.from({ length: 120 }, () => group).flat();
  const heavy = parts.join(', ');
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate(
      (source) => window.promptBrowserProof.mount('nodes-2', 0.85, source),
      heavy,
    );
    await ready(page);
    await dragChip(page, 2, 0);
    const expected = [parts[0], parts[2], parts[1], ...parts.slice(3)].join(', ');
    await page.waitForFunction(
      (expected) => window.promptBrowserProof.read().source === expected,
      expected,
    );
    await page.keyboard.press('Control+z');
    await page.waitForFunction(
      (expected) => window.promptBrowserProof.read().source === expected,
      heavy,
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } catch (cause) {
    await page.screenshot({ path: 'D:/SugarCubes/prompt-editor/heavy-drag-failure.png' });
    throw cause;
  } finally {
    await page.close();
  }
}, 60_000);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s promoted prompts retain external values, permissions and clean teardown',
  async (renderer) => {
    const page = await browser.newPage();
    try {
      await page.goto(address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, source }) => window.promptBrowserProof.mount(renderer, 1, source, true),
        { renderer, source },
      );
      await ready(page);
      await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
      await page.keyboard.press('Control+End');
      await page.keyboard.insertText(', boundary edit');
      await page.waitForFunction(
        (expected) => window.promptBrowserProof.read().source === expected,
        source + ', boundary edit',
      );
      const replacement = 'external <b>literal</b>\r\n(bright sky:1.30), {colors}';
      await page.evaluate((value) => window.promptBrowserProof.replace(value), replacement);
      await page.waitForFunction(() =>
        document.querySelector('.cm-content')?.textContent?.includes('external'),
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(replacement);
      await page.evaluate(() => window.promptBrowserProof.setReadOnly(true));
      await page.waitForFunction(
        () => document.querySelector('.cm-content')?.getAttribute('aria-readonly') === 'true',
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(1);
      await page.evaluate(() => window.promptBrowserProof.reset());
      expect(await page.locator('[data-sc-prompt-editor]').count()).toBe(0);
      expect(
        await page
          .locator('textarea:not([data-sp-owned])')
          .evaluate((element) => (element as HTMLTextAreaElement).hidden),
      ).toBe(false);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);

test('native composition survives repeated host refresh and publishes only committed text', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
    await page.keyboard.press('Control+End');
    const input = await page.context().newCDPSession(page);
    await input.send('Input.imeSetComposition', { text: '漢', selectionStart: 1, selectionEnd: 1 });
    await page.evaluate(() => {
      for (let i = 0; i < 5; i++) window.promptBrowserProof.refresh();
    });
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
    expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
    await input.send('Input.insertText', { text: '漢字' });
    await page.waitForFunction(
      (expected) => window.promptBrowserProof.read().source === expected,
      source + '漢字',
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 60_000);

test('native field annotations preserve the focused editor and its editing identity', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
    await page.evaluate(() => window.promptBrowserProof.annotateNativeField());
    expect(
      await page.evaluate(() => document.activeElement?.classList.contains('cm-content')),
    ).toBe(true);
  } finally {
    await page.close();
  }
}, 60_000);

test('catalog changes repaint installed LoRA metadata without editing the prompt', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    await page.evaluate(() => window.promptBrowserProof.updateModelTitle('Updated model'));
    await page.waitForFunction(
      () => document.querySelector('.sp-lora-title')?.textContent === 'Updated model',
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
    expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 60_000);

test('unchanged LoRA catalog coverage follows source edits through raw and rich rendering', async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(5_000);
  let stage = 'initial banner';
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((value) => window.promptBrowserProof.mount('nodes-2', 1, value), source);
    await ready(page);
    stage = 'edit source';
    await page.locator('.cm-content [data-reorder-chip="0"]').first().click();
    await page.keyboard.press('Control+End');
    await page.keyboard.insertText(', edited tail');
    await page.waitForFunction(
      (value) => window.promptBrowserProof.read().source === value + ', edited tail',
      source,
    );
    const menu = page.locator('[data-sc-prompt-menu]');
    for (let toggle = 0; toggle < 2; toggle++) {
      stage = toggle === 0 ? 'switch to raw' : 'switch to rich';
      await page
        .locator(toggle === 0 ? '.cm-content [data-reorder-chip="0"]' : '.cm-content')
        .first()
        .click({ button: 'right' });
      await menu.waitFor({ state: 'visible' });
      await menu.getByRole('menuitemcheckbox', { name: /rich|decoration/i }).click();
      if (toggle === 0) await page.locator('.sp-lora-chip').waitFor({ state: 'hidden' });
    }
    stage = 'restored banner';
    await ready(page);
    await page.locator('.sp-lora-chip[data-media="ready"]').scrollIntoViewIfNeeded();
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(
      source + ', edited tail',
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } catch (cause) {
    const media = await page.locator('.sp-lora-chip').evaluateAll((nodes) =>
      nodes.map((node) => ({
        resolution: node.getAttribute('data-resolution'),
        media: node.getAttribute('data-media'),
      })),
    );
    throw new Error(`LoRA source/media proof failed at ${stage}: ${JSON.stringify(media)}`, {
      cause,
    });
  } finally {
    await page.close();
  }
}, 30_000);

test('native drag axes accept Comfy scale3d and reject rotated, reflected and perspective contexts', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    expect(
      await page.evaluate(() =>
        window.promptBrowserProof.axes('matrix3d(.85,0,0,0,0,.85,0,0,0,0,.85,0,60,40,0,1)'),
      ),
    ).toEqual({ x: 0.85, y: 0.85 });
    for (const transform of ['rotate(10deg)', 'scale(-1,1)', 'perspective(600px) rotateY(15deg)'])
      expect(
        await page.evaluate((transform) => window.promptBrowserProof.axes(transform), transform),
      ).toBeNull();
  } finally {
    await page.close();
  }
}, 60_000);

test('wrapped decorated prompts retain native LoRA pickup and drop in a narrow Cube card', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.addStyleTag({ content: '.lg-node{width:240px!important}' });
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    await dragChip(page, 2, 4, true);
    await page.waitForFunction(
      () =>
        window.promptBrowserProof.read().source ===
        'a cat, (bright eyes:1.2), {colors}, tail, <lora:example:0.8>',
    );
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } catch (cause) {
    await page.screenshot({ path: 'D:/SugarCubes/prompt-editor/wrapped-drag-failure.png' });
    throw cause;
  } finally {
    await page.close();
  }
}, 60_000);

test('shared prompt menu toggles decoration without changing source and dismisses outside', async () => {
  const page = await browser.newPage();
  try {
    await page.goto(address);
    await page.waitForFunction(() => !!window.promptBrowserProof);
    await page.evaluate((source) => window.promptBrowserProof.mount('nodes-2', 1, source), source);
    await ready(page);
    const chip = page.locator('.cm-content [data-reorder-chip="0"]').first();
    await chip.click({ button: 'right' });
    const menu = page.locator('[data-sc-prompt-menu]');
    await menu.waitFor({ state: 'visible' });
    const toggle = menu.getByRole('menuitemcheckbox', { name: /rich|decoration/i });
    await toggle.click();
    await page.locator('.sp-lora-chip').waitFor({ state: 'hidden' });
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
    expect(await page.evaluate(() => window.promptBrowserProof.read().callbacks)).toBe(0);
    await page.locator('.cm-content').click({ button: 'right' });
    await menu.waitFor({ state: 'visible' });
    await page.mouse.click(2, 2);
    await menu.waitFor({ state: 'hidden' });
    await page.locator('.cm-content').click({ button: 'right' });
    await menu.waitFor({ state: 'visible' });
    await menu.getByRole('menuitemcheckbox', { name: /rich|decoration/i }).click();
    await ready(page);
    await page.locator('.sp-lora-chip[data-media="ready"]').scrollIntoViewIfNeeded();
    expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe(source);
    expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
  } finally {
    await page.close();
  }
}, 60_000);
