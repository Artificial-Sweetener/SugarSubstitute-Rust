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
 * The generation queue's geometry, which is SugarSubstitute's own and the same in every visual
 * style, drawn with the active style's tokens so colours, fonts and radii follow that style.
 */

/** The fly-out's fixed width. */
export const GENERATION_QUEUE_FLYOUT_WIDTH = 360;
/** The fly-out's greatest height; a longer list scrolls inside it. */
export const GENERATION_QUEUE_FLYOUT_MAX_HEIGHT = 420;
/** The callout arrow's height above the fly-out. */
export const GENERATION_QUEUE_ARROW_HEIGHT = 7;
/** Half the width of the callout arrow's base. */
export const GENERATION_QUEUE_ARROW_HALF_WIDTH = 8;

const Q = '.sc-generation-queue';
const ROW = '.sc-generation-queue-row';
const LIGHT = `${Q}[data-sc-color-scheme='light']`;
/** A glyph drawn as a mask in the current text colour. */
const glyph = (path: string): string =>
  `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='${path}' fill='none' stroke='black' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") center/16px 16px no-repeat`;
const CLOSE = glyph('M4 4l8 8M12 4l-8 8');
const DELETE = glyph('M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5M6.8 7v4M9.2 7v4');
const MORE = glyph('M3.5 8h.01M8 8h.01M12.5 8h.01');
const PLAY = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='M6 4.5l5.5 3.5L6 11.5z' fill='white'/%3E%3C/svg%3E") center/16px 16px no-repeat`;
const PANEL = glyph('M2.5 3.5h11v9h-11zM10 3.5v9');

/** The rules for the generation queue's own view. */
export const generationQueueViewCss = `
  ${Q} { box-sizing:border-box; display:flex; flex-direction:column; min-height:0; color:var(--sc-text,#fff);
    font-family:var(--sc-font-family); --sc-gq-row:rgba(255,255,255,.0706); --sc-gq-row-border:rgba(255,255,255,.0863);
    --sc-gq-row-border-active:rgba(255,255,255,.1098); --sc-gq-thumb:rgba(255,255,255,.0627);
    --sc-gq-thumb-border:rgba(255,255,255,.0784); --sc-gq-over:255,255,255; --sc-gq-rule:rgba(255,255,255,.1765); }
  ${LIGHT} { --sc-gq-row:rgba(255,255,255,.8824); --sc-gq-row-border:rgba(0,0,0,.0784);
    --sc-gq-row-border-active:rgba(0,0,0,.1255); --sc-gq-thumb:rgba(0,0,0,.0314); --sc-gq-thumb-border:rgba(0,0,0,.0706);
    --sc-gq-over:0,0,0; --sc-gq-rule:rgba(0,0,0,.1765); }
  ${Q}[data-surface='flyout'] { width:${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px; max-height:${String(GENERATION_QUEUE_FLYOUT_MAX_HEIGHT)}px;
    padding:12px 14px 14px; gap:10px; overflow:visible!important; border:1px solid var(--sc-border-subtle,rgba(0,0,0,.18));
    border-radius:8px; background:var(--sc-surface-secondary,#282828); box-shadow:0 8px 35px rgba(0,0,0,.31); }
  ${LIGHT}[data-surface='flyout'] { box-shadow:0 8px 35px rgba(0,0,0,.12); }
  /* The callout's arrow points at the queue button; its owner sets where along the top edge. */
  ${Q}[data-surface='flyout']::before { content:''; position:absolute; top:-${String(GENERATION_QUEUE_ARROW_HEIGHT)}px;
    left:calc(var(--sc-gq-arrow,50%) - ${String(GENERATION_QUEUE_ARROW_HALF_WIDTH)}px); width:${String(GENERATION_QUEUE_ARROW_HALF_WIDTH * 2)}px;
    height:${String(GENERATION_QUEUE_ARROW_HEIGHT)}px; background:var(--sc-border-subtle,rgba(0,0,0,.18)); clip-path:polygon(50% 0,100% 100%,0 100%); }
  ${Q}[data-surface='flyout']::after { content:''; position:absolute; top:-${String(GENERATION_QUEUE_ARROW_HEIGHT - 1.5)}px;
    left:calc(var(--sc-gq-arrow,50%) - ${String(GENERATION_QUEUE_ARROW_HALF_WIDTH)}px); width:${String(GENERATION_QUEUE_ARROW_HALF_WIDTH * 2)}px;
    height:${String(GENERATION_QUEUE_ARROW_HEIGHT)}px; background:var(--sc-surface-secondary,#282828); clip-path:polygon(50% 0,100% 100%,0 100%); }
  ${Q}[data-surface='panel'] { width:100%; height:100%; padding:12px; gap:10px; }
  .sc-generation-queue-header { display:flex; align-items:center; gap:8px; min-height:20px; }
  .sc-generation-queue-title { flex:1; min-width:0; margin:0; font:600 14px/20px var(--sc-font-family); color:inherit;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sc-generation-queue-summary { flex:0 0 auto; font:400 12px/16px var(--sc-font-family); white-space:nowrap; }
  .sc-generation-queue-summary[hidden] { display:none; }
  ${Q} :is(.sc-generation-queue-more,.sc-generation-queue-hide,.sc-generation-queue-row-action) { flex:0 0 28px; width:28px; height:28px;
    padding:0; border:0; border-radius:var(--sc-control-radius,4px); background:transparent; color:inherit; cursor:pointer; position:relative; }
  ${Q} :is(.sc-generation-queue-more,.sc-generation-queue-hide,.sc-generation-queue-row-action)::before { content:''; position:absolute; inset:0;
    background:currentColor; }
  .sc-generation-queue-more::before { -webkit-mask:${MORE}; mask:${MORE}; }
  .sc-generation-queue-hide::before { -webkit-mask:${PANEL}; mask:${PANEL}; }
  .sc-generation-queue-row-action[data-action='cancel']::before { -webkit-mask:${CLOSE}; mask:${CLOSE}; }
  .sc-generation-queue-row-action[data-action='remove']::before { -webkit-mask:${DELETE}; mask:${DELETE}; }
  ${Q} :is(.sc-generation-queue-more,.sc-generation-queue-hide,.sc-generation-queue-row-action):hover { background:rgba(var(--sc-gq-over),.0353); }
  ${Q} :is(.sc-generation-queue-more,.sc-generation-queue-hide,.sc-generation-queue-row-action):active { background:rgba(var(--sc-gq-over),.0235); }
  ${Q} :is(.sc-generation-queue-more,.sc-generation-queue-hide,.sc-generation-queue-row-action):focus-visible {
    outline:2px solid var(--sc-brand-border,var(--sc-accent-color,#ff4089)); outline-offset:-2px; }
  .sc-generation-queue-search { box-sizing:border-box; width:100%; height:32px; padding:0 10px; border:1px solid var(--sc-control-stroke,rgba(255,255,255,.07));
    border-radius:var(--sc-control-radius,4px); background:var(--sc-field-fill,rgba(255,255,255,.06)); color:inherit; font:400 14px/20px var(--sc-font-family); }
  .sc-generation-queue-search:focus-visible { outline:2px solid var(--sc-brand-border,var(--sc-accent-color,#ff4089)); outline-offset:-1px; }
  .sc-generation-queue-empty { display:flex; align-items:center; justify-content:center; flex:1; min-height:88px; margin:0;
    font:400 14px/20px var(--sc-font-family); }
  .sc-generation-queue-empty[hidden], .sc-generation-queue-rows[hidden] { display:none; }
  .sc-generation-queue-rows { display:flex; flex-direction:column; gap:6px; flex:1; min-height:0; overflow-x:hidden; overflow-y:auto; }
  ${ROW} { box-sizing:border-box; position:relative; display:flex; align-items:center; gap:8px; flex:0 0 68px; height:68px; min-width:0;
    padding:8px 6px 8px 10px; border:1px solid var(--sc-gq-row-border); border-radius:6px; background:var(--sc-gq-row); }
  ${ROW}[data-role='active'] { border-color:var(--sc-gq-row-border-active); }
  /* Waiting rows answer the pointer; the running row stays lit; finished rows are still. */
  ${ROW}::after { content:''; position:absolute; inset:1px; border-radius:6px; pointer-events:none; background:rgb(var(--sc-gq-over)); opacity:0;
    transition:opacity 83ms linear; }
  ${ROW}:is([data-role='active'],[data-role='waiting']:hover)::after { opacity:.098; }
  ${ROW}[data-role='waiting']:active::after { opacity:.2; }
  @media (prefers-reduced-motion:reduce) { ${ROW}::after { transition:none; } }
  .sc-generation-queue-thumbnail { box-sizing:border-box; flex:0 0 52px; width:52px; height:52px; overflow:hidden; border:1px solid var(--sc-gq-thumb-border);
    border-radius:5px; background:var(--sc-gq-thumb); }
  .sc-generation-queue-thumbnail > img { display:block; width:100%; height:100%; object-fit:cover; }
  ${ROW}[data-status='completed'] .sc-generation-queue-thumbnail:has(img) { cursor:pointer; }
  .sc-generation-queue-thumbnail { position:relative; }
  ${ROW}[data-video] .sc-generation-queue-thumbnail::after { content:''; position:absolute; right:3px; bottom:3px; width:16px; height:16px;
    border-radius:50%; background:rgba(0,0,0,.6) ${PLAY}; }
  .sc-generation-queue-text { flex:1; min-width:0; display:flex; flex-direction:column; justify-content:center; }
  .sc-generation-queue-row-title, .sc-generation-queue-row-status { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; color:inherit; }
  .sc-generation-queue-row-title { font:600 14px/19px var(--sc-font-family); }
  .sc-generation-queue-row-status { font:400 12px/16px var(--sc-font-family); }
  .sc-generation-queue-row-action { z-index:1; }
  .sc-generation-queue-caption { flex:0 0 auto; display:flex; align-items:center; gap:8px; padding:4px 2px 0; font:400 12px/16px var(--sc-font-family); }
  .sc-generation-queue-caption::after { content:''; flex:1; height:1px; background:var(--sc-gq-rule); }
  @media (forced-colors:active) { ${Q}[data-surface='flyout'] { background:Canvas; border-color:CanvasText; } ${ROW} { border-color:CanvasText; } }
`;
