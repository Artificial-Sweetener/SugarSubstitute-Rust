// frontend/comfyui/ui/core/SegmentedSlider.ts
function segmentedSliderCss(list, indicator) {
  return `
    ${list} { position:relative; display:flex; align-items:center; gap:0; margin:0; padding:0; overflow-x:auto;
      min-width:0; white-space:nowrap; overscroll-behavior:contain; scrollbar-width:thin; }
    ${list} button { position:relative; background:transparent; border:1px solid transparent; border-radius:5px;
      padding:0 12px; height:30px; min-height:30px; box-shadow:none; font-size:14px; line-height:28px; color:inherit;
      font-family:inherit; cursor:default; }
    ${list} button:hover:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 3.5%,transparent); }
    ${list} button:active:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 2.5%,transparent); }
    ${list} button[aria-selected="true"] { box-shadow:none; border-color:transparent; background:transparent; }
    ${indicator} { position:absolute; left:0; top:0; height:30px; box-sizing:border-box; pointer-events:none;
      border:1px solid color-mix(in srgb,var(--fg-color,#fff) 5.5%,transparent); border-radius:5px;
      background:color-mix(in srgb,var(--fg-color,#fff) 6%,transparent); }
    ${indicator}[hidden] { display:none; }
    ${indicator}::after { content:""; position:absolute; bottom:0;
      left:calc(50% - 8px); width:16px; height:3px; border-radius:1.5px; background:var(--p-primary-color,var(--ring,#df258e)); }
    ${indicator}[data-placed] { transition:transform 200ms var(--sc-motion-ease,cubic-bezier(0,0,0,1)),
      width 200ms var(--sc-motion-ease,cubic-bezier(0,0,0,1)); }
    @media (prefers-reduced-motion:reduce) { ${indicator}[data-placed] { transition:none; } }`;
}
function placeSegmentIndicator(indicator, tab) {
  if (!tab) {
    indicator.hidden = true;
    delete indicator.dataset.placed;
    return;
  }
  indicator.hidden = false;
  indicator.style.transform = `translateX(${String(tab.offsetLeft)}px)`;
  indicator.style.width = `${String(tab.offsetWidth)}px`;
  if (!indicator.dataset.placed) {
    void indicator.offsetWidth;
    indicator.dataset.placed = "true";
  }
}
function segmentKeyTarget(event, index, count) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || !count) return null;
  if (event.key === "Home") return 0;
  if (event.key === "End") return count - 1;
  if (event.key === "ArrowRight") return (index + 1) % count;
  if (event.key === "ArrowLeft") return (index + count - 1) % count;
  return null;
}

export {
  segmentedSliderCss,
  placeSegmentIndicator,
  segmentKeyTarget
};
