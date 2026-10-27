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
/** Match the supplied Photoshop tab and scrollbar geometry without changing other styles. */
import { chromium, type Browser } from '@playwright/test';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { photoProControlCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProAppearanceStyles.js';
import {
  photoProTokenDeclarations,
  photoProScrollbarTokenDeclarations,
} from '../../../frontend/comfyui/ui/surface/appearance/PhotoProThemeTokens.js';
import {
  photoProScrollbarCss,
  scrollbarPresentationCss,
} from '../../../frontend/comfyui/ui/surface/appearance/ScrollbarStyles.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';

let browser: Browser;
beforeAll(async () => {
  browser = await launchHeadlessBrowser(chromium);
});
afterAll(async () => {
  await browser.close();
});

test('matches edge-aligned 30px tabs, 16px tracks and 12px thumbs and uses only the selected cube fill', async () => {
  const page = await browser.newPage();
  try {
    await page.setContent(`<!doctype html><html data-sc-visual-style="photopro" data-sc-photopro-palette="dark-gray" data-sc-substitute-frame><body>
      <header class="sc-substitute-frame" data-sc-theme="photopro"><div class="sc-workflow-strip"><div data-sc-workflow-tabs><button data-sc-workflow-tab aria-selected="false">Other<span class="sc-workflow-close">�</span></button><button data-sc-workflow-tab aria-selected="true">Selected</button></div></div><button class="sc-workflow-new" data-sc-button>+</button></header>
      <nav class="sc-cube-stack"><div class="sc-cube-stack__selection"></div></nav>
      <section data-sc-style-surface><div class="sc-style-scrollbar" data-sc-scrollbar-style="photopro" aria-orientation="vertical" style="width:16px;height:100px"><div class="sc-style-scrollbar-track" style="width:16px;height:300px"><button data-sc-scroll-step="start"></button><button data-sc-scroll-step="end"></button></div></div></section>
      </body></html>`);
    await page.addStyleTag({
      content: `body {margin:0;} :root {${photoProScrollbarTokenDeclarations('dark-gray')}} [data-sc-theme="photopro"] {${photoProTokenDeclarations('dark-gray')}} ${substituteFrameCss}${photoProControlCss()}${scrollbarPresentationCss()}${photoProScrollbarCss()}${photoProControlCss()}`,
    });
    const actual = await page.evaluate(() => {
      const frame = document.querySelector('header')!;
      const tabs = [...document.querySelectorAll('[data-sc-workflow-tab]')];
      const scrollbar = document.querySelector('.sc-style-scrollbar')!;
      const arrow = document.querySelector('[data-sc-scroll-step]')!;
      return {
        frameHeight: frame.getBoundingClientRect().height,
        frameFill: getComputedStyle(frame).backgroundColor,
        closeGlyphWidth: getComputedStyle(document.querySelector('.sc-workflow-close')!, '::after')
          .width,
        closeGlyphHeight: getComputedStyle(document.querySelector('.sc-workflow-close')!, '::after')
          .height,
        plus: {
          top: document.querySelector('.sc-workflow-new')!.getBoundingClientRect().top,
          height: document.querySelector('.sc-workflow-new')!.getBoundingClientRect().height,
          radius: getComputedStyle(document.querySelector('.sc-workflow-new')!).borderRadius,
          border: getComputedStyle(document.querySelector('.sc-workflow-new')!).borderTopWidth,
          fill: getComputedStyle(document.querySelector('.sc-workflow-new')!).backgroundColor,
        },
        tabs: tabs.map((tab) => ({
          top: tab.getBoundingClientRect().top,
          height: tab.getBoundingClientRect().height,
          fill: getComputedStyle(tab).backgroundColor,
          color: getComputedStyle(tab).color,
          radius: getComputedStyle(tab).borderRadius,
        })),
        thumbWidth: getComputedStyle(scrollbar, '::before').width,
        thumbFill: getComputedStyle(scrollbar, '::before').backgroundColor,
        thumbBorder: getComputedStyle(scrollbar, '::before').borderTopWidth,
        trackFill: getComputedStyle(document.querySelector('.sc-style-scrollbar-track')!)
          .backgroundColor,
        arrowColor: getComputedStyle(arrow).color,
        arrowSizing: getComputedStyle(arrow, '::before').boxSizing,
        arrowTop: getComputedStyle(arrow, '::before').top,
        marker: getComputedStyle(document.querySelector('.sc-cube-stack__selection')!).display,
      };
    });
    expect(actual).toEqual({
      frameHeight: 30,
      frameFill: 'rgb(50, 50, 50)',
      closeGlyphWidth: '6px',
      closeGlyphHeight: '6px',
      plus: { top: 0, height: 30, radius: '0px', border: '0px', fill: 'rgba(0, 0, 0, 0)' },
      tabs: [
        { top: 0, height: 30, fill: 'rgb(38, 38, 38)', color: 'rgb(135, 135, 135)', radius: '0px' },
        { top: 0, height: 30, fill: 'rgb(50, 50, 50)', color: 'rgb(225, 225, 225)', radius: '0px' },
      ],
      thumbWidth: '12px',
      thumbFill: 'rgb(71, 71, 71)',
      thumbBorder: '0px',
      trackFill: 'rgb(41, 41, 41)',
      arrowColor: 'rgb(121, 121, 121)',
      arrowSizing: 'content-box',
      arrowTop: '9px',
      marker: 'none',
    });
    await page.locator('.sc-workflow-close').hover();
    const highlighted = await page.locator('.sc-workflow-close').evaluate((e) => {
      const s = getComputedStyle(e),
        b = e.getBoundingClientRect();
      return {
        width: b.width,
        height: b.height,
        border: s.borderColor,
        color: s.color,
        tabFill: getComputedStyle(e.parentElement!).backgroundColor,
      };
    });
    expect(highlighted).toEqual({
      width: 14,
      height: 12,
      border: 'rgb(71, 71, 71)',
      color: 'rgb(128, 128, 128)',
      tabFill: 'rgb(46, 46, 46)',
    });
    await page.locator('.sc-style-scrollbar').evaluate((e) => {
      if (!(e instanceof HTMLElement)) throw new Error('The scrollbar must be an HTML element.');
      const element = e;
      element.style.setProperty('--sc-scrollbar-scale-x', '2');
      element.style.setProperty('--sc-scrollbar-scale-y', '2');
      element.style.transform = 'scale(2)';
    });
    expect(
      await page.locator('[data-sc-scroll-step="start"]').evaluate((e) => ({
        width: e.getBoundingClientRect().width,
        height: e.getBoundingClientRect().height,
      })),
    ).toEqual({ width: 16, height: 16 });
    await page.evaluate(() => (document.documentElement.dataset.scVisualStyle = 'fluent'));
    expect(
      await page.locator('.sc-cube-stack__selection').evaluate((e) => getComputedStyle(e).display),
    ).toBe('block');
  } finally {
    await page.close();
  }
});
