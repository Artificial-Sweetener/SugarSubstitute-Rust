// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayStyles.ts
function ensureCanvasOverlayStyles(document) {
  if (document.getElementById("sugarcubes-output-overlay-styles")) return;
  const style = document.createElement("style");
  style.id = "sugarcubes-output-overlay-styles";
  style.textContent = `
    .sc-output-overlay-root,.sc-output-overlay { position:absolute; inset:0; pointer-events:none; }
    .sc-output-overlay { color:var(--fg-color,#ddd); font:13px var(--comfy-font-family,system-ui,sans-serif); }
    .sc-output-tabstrip,.sc-output-status {
      pointer-events:auto; display:flex; align-items:center; gap:4px;
      border:1px solid var(--interface-stroke,#454545); border-radius:8px;
      background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020));
      box-shadow:0 2px 8px #0003; box-sizing:border-box;
    }
    .sc-output-tabstrip { position:absolute; bottom:8px; left:8px;
      max-width:calc(100% - 16px); padding:4px; z-index:2; }
    .sc-output-tabs { display:flex; align-items:center; gap:0; overflow-x:auto; min-width:0;
      white-space:nowrap; overscroll-behavior:contain; scrollbar-width:thin; }
    .sc-output-batch-navigation { display:flex; align-items:center; flex-shrink:0; }
    .sc-output-batch-navigation:empty { display:none; }
    .sc-output-batch { font-size:12px; padding:0 4px; white-space:nowrap; }
    .sc-output-status { position:absolute; bottom:60px; left:12px; right:12px; padding:6px;
      flex-wrap:wrap; font-size:12px; z-index:2; }
    .sc-output-overlay button,.sc-output-overlay select,.sc-output-overlay input {
      color:inherit; font:inherit; background:var(--comfy-input-bg,#303030);
      border:1px solid var(--interface-stroke,#454545); border-radius:5px; min-height:28px;
      padding:4px 8px; box-sizing:border-box;
    }
    .sc-output-overlay button { cursor:pointer; flex-shrink:0; }
    .sc-output-overlay button:hover:not(:disabled) { background:var(--secondary-background-hover,#444); }
    .sc-output-overlay button:disabled { opacity:.45; cursor:default; }
    .sc-output-overlay button[aria-pressed="true"],.sc-output-overlay [aria-selected="true"] {
      border-color:var(--p-primary-color,var(--ring,#df258e));
      box-shadow:inset 0 -2px var(--p-primary-color,var(--ring,#df258e));
    }
    .sc-output-overlay :focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:2px; }
    .sc-output-status span:empty { display:none; }
    .sc-output-wipe { position:absolute; pointer-events:auto; touch-action:none; z-index:1;
      display:flex; align-items:center; justify-content:center; background:#fff8; outline:none; }
    .sc-output-wipe[data-orientation="vertical"] { top:0; bottom:0; width:4px; transform:translateX(-50%); cursor:ew-resize; }
    .sc-output-wipe[data-orientation="horizontal"] { left:0; right:0; height:4px; transform:translateY(-50%); cursor:ns-resize; }
    .sc-output-wipe span { border-radius:20px; padding:8px; color:#111; background:#eee; box-shadow:0 1px 6px #0008; }
    .sc-output-tabs button { position:relative; background:transparent; border:1px solid transparent; border-radius:5px;
      padding:6px 12px; min-height:30px; box-shadow:none; font-size:14px; }
    .sc-output-tabs button:hover:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 3.5%,transparent); }
    .sc-output-tabs button:active:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 2.5%,transparent); }
    .sc-output-tabs button[aria-selected="true"] { box-shadow:none;
      border-color:color-mix(in srgb,var(--fg-color,#fff) 5.5%,transparent);
      background:color-mix(in srgb,var(--fg-color,#fff) 6%,transparent); }
    .sc-output-tabs button[aria-selected="true"]::after { content:""; position:absolute; bottom:0;
      left:calc(50% - 8px); width:16px; height:3px; border-radius:1.5px; background:var(--p-primary-color,var(--ring,#df258e)); }
    .sc-output-overlay .sc-output-batch-arrow { border:0; background:transparent; padding:2px 6px; }
    .sc-output-loading { position:absolute; left:50%; top:50%; transform:translate(-50%,-50%);
      pointer-events:none; z-index:2; display:grid; place-items:center; }
    .sc-output-spinner { width:28px; height:28px; box-sizing:border-box; border-radius:50%;
      border:3px solid color-mix(in srgb,var(--fg-color,#ddd) 25%,transparent);
      border-top-color:var(--p-primary-color,var(--ring,#df258e)); animation:sc-output-spin .9s linear infinite; }
    @keyframes sc-output-spin { to { transform:rotate(360deg); } }
    @media (prefers-reduced-motion:reduce) { .sc-output-spinner { animation:none; } }

    .sc-output-announcement { position:absolute; width:1px; height:1px; padding:0; margin:-1px;
      overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  `;
  document.head.append(style);
}

export {
  ensureCanvasOverlayStyles
};
