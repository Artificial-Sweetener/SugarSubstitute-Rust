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
 * Dress Comfy's own job queue in the generation queue's Fluent look.
 *
 * The queue stays Comfy's component: every tab, filter, sort, row action and menu it offers is
 * the one a person uses. These rules change only how its fly-out and its job list look inside
 * the SugarSubstitute frame: the surface, the title, and rows with the larger thumbnail, strong
 * title and caption line of the generation queue.
 *
 * The fly-out is opaque. It opens over the native canvas, whose pixels the page cannot blur, so
 * a translucent panel would show the image sharp behind its text.
 */
/** Comfy's queue fly-out. */
const FLYOUT = ':root[data-sc-substitute-frame] [data-testid="queue-progress-overlay"]';
/** The job list Comfy shows in its fly-out and in its docked panel. */
const LIST = ':root[data-sc-substitute-frame] [data-testid="job-assets-list"]';
/** Comfy's docked job panel, and the side panel and splitter that hold it. */
const DOCK = ':root[data-sc-substitute-frame] [data-testid="job-history-sidebar"]';
const DOCK_PANEL = ':root[data-sc-substitute-frame] .side-bar-panel:has([data-testid="job-history-sidebar"])';
/** One job row: Comfy gives each a fixed-height slot holding the row itself. */
const ROW = `${LIST} .h-12 > div`;
/** The queue's width as a fly-out, as the generation queue has always been. */
export const GENERATION_QUEUE_FLYOUT_WIDTH = 360;
/**
 * How far the job list is enlarged. Comfy lays rows out in fixed 48 px slots it positions
 * itself, so a row cannot simply be made taller; the whole list is scaled until a slot is the
 * generation queue's 68 px row and its 6 px gap.
 */
export const GENERATION_QUEUE_LIST_SCALE = 74 / 48;
/** Write a length of the finished look in the scaled list's own pixels. */
const px = (value) => `calc(${String(value)}px / var(--sc-queue-scale))`;
/** Mix the text colour into transparency, which reads right on dark and light surfaces. */
const tint = (percent) => `color-mix(in srgb,var(--sc-text,#fff) ${String(percent)}%,transparent)`;
/** The rules that give Comfy's job queue the generation queue's look inside the frame. */
export const generationQueueSkinCss = `
  ${FLYOUT} { width:${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px!important; min-width:0!important; max-height:min(60vh,520px)!important;
    box-sizing:border-box; border:1px solid var(--sc-border-subtle,${tint(8.6)})!important; border-radius:8px!important;
    background:var(--sc-surface-secondary,#2b2b2b)!important; color:var(--sc-text,#fff);
    -webkit-backdrop-filter:none!important; backdrop-filter:none!important;
    box-shadow:var(--sc-shadow-medium,0 8px 16px rgba(0,0,0,.26))!important; font-family:var(--sc-font-family); }
  @media (forced-colors:active) { ${FLYOUT} { background:Canvas!important; border-color:CanvasText!important; } }
  /* The header keeps Comfy's controls; its title reads as the generation queue's. */
  ${FLYOUT} > div > div:first-child { height:44px; padding:0 8px 0 6px; border-bottom-color:var(--sc-border-subtle,${tint(8.6)}); }
  ${FLYOUT} > div > div:first-child > div:first-child { font-size:0!important; }
  ${FLYOUT} > div > div:first-child > div:first-child::before { content:'Generation queue';
    font:600 14px/20px var(--sc-font-family); color:var(--sc-text,#fff); }
  ${FLYOUT} > div { gap:10px!important; }
  ${LIST} { --sc-queue-scale:${String(GENERATION_QUEUE_LIST_SCALE)}; zoom:var(--sc-queue-scale);
    font-family:var(--sc-font-family); padding-bottom:${px(8)}!important; }
  ${LIST} > div > .px-3 { padding:0 ${px(14)}!important; }
  /* Day headings read as the queue's caption dividers. */
  ${LIST} > div > .text-xs { display:flex; align-items:center; gap:${px(8)}; padding:0 ${px(14)} ${px(6)}!important;
    font:400 ${px(11)}/${px(14)} var(--sc-font-family)!important; color:var(--sc-text-secondary,${tint(78.6)})!important; }
  ${LIST} > div > .text-xs::after { content:''; flex:1; height:${px(1)}; background:${tint(17.6)}; }
  ${ROW} { box-sizing:border-box; height:calc(100% - ${px(6)})!important; padding:${px(8)} ${px(6)} ${px(8)} ${px(10)}!important;
    gap:${px(8)}!important; border:${px(1)} solid ${tint(8.6)}; border-radius:${px(6)}!important; background:${tint(7)}!important; }
  ${ROW}:hover { border-color:${tint(11)}; background:${tint(9.4)}!important; }
  /* A running job fills its row with the accent as it advances. */
  ${ROW} > .absolute > div { background:color-mix(in srgb,var(--sc-accent-color,#ff4089) 28%,transparent)!important; }
  ${ROW} > .size-8 { width:${px(52)}!important; height:${px(52)}!important; flex:0 0 ${px(52)}; border-radius:${px(5)}!important;
    border:${px(1)} solid ${tint(7.8)}; background:${tint(6.3)}!important; }
  ${ROW} > .flex-col { gap:${px(3)}!important; }
  ${ROW} > .flex-col > div:first-child { font:600 ${px(14)}/${px(18)} var(--sc-font-family)!important; color:var(--sc-text,#fff)!important; }
  ${ROW} > .flex-col > div:nth-child(2) { font:400 ${px(12)}/${px(16)} var(--sc-font-family)!important;
    color:var(--sc-text-secondary,${tint(78.6)})!important; }
  /* Comfy's own row buttons keep their real size inside the enlarged list. */
  ${ROW} button { zoom:calc(1 / var(--sc-queue-scale)); font-family:var(--sc-font-family); }
  /* Docked, the queue is Comfy's side panel: the frame shows that one panel, pinned at the right at
     the fly-out's width, and the queue button steps aside so the queue is never offered in two places. */
  ${DOCK_PANEL} { display:flex!important; order:3; flex:0 0 ${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px!important;
    width:${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px; min-width:0!important; box-sizing:border-box;
    background:var(--sc-surface-secondary,#2b2b2b)!important; border-left:1px solid var(--sc-border-subtle,${tint(8.6)});
    color:var(--sc-text,#fff); font-family:var(--sc-font-family); }
  :root[data-sc-substitute-frame]:has([data-testid="job-history-sidebar"]) :is(button[data-sc-generation-queue],.sc-generation-queue-slot) {
    display:none!important; }
  :root[data-sc-substitute-frame]:has([data-testid="job-history-sidebar"]) .sc-generation-strip { justify-content:flex-end; }
  :root[data-sc-substitute-frame]:has([data-testid="job-history-sidebar"]) .sc-generation-actions { width:120px; }
  ${DOCK} .p-toolbar { min-height:44px!important; padding:0 8px 0 14px!important; border-bottom-color:var(--sc-border-subtle,${tint(8.6)})!important; }
  ${DOCK} .p-toolbar-start > span { font-size:0!important; }
  ${DOCK} .p-toolbar-start > span::before { content:'Generation queue'; font:600 14px/20px var(--sc-font-family); color:var(--sc-text,#fff); }
  .sc-generation-queue-dock-hide { display:inline-flex; align-items:center; justify-content:center; width:28px; height:28px; padding:0;
    margin-left:2px; border:0; border-radius:var(--sc-control-radius,4px); background:transparent; color:inherit; cursor:pointer; }
  .sc-generation-queue-dock-hide::before { content:''; width:10px; height:10px; background:currentColor;
    clip-path:polygon(0 8%,8% 0,50% 42%,92% 0,100% 8%,58% 50%,100% 92%,92% 100%,50% 58%,8% 100%,0 92%,42% 50%); }
  .sc-generation-queue-dock-hide:hover { background:${tint(9.4)}; }
  .sc-generation-queue-dock-hide:active { background:${tint(6.3)}; }
`;
