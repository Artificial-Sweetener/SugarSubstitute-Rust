// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Embedded only by the off-screen native qualification entry.
(() => {
  const send = (operation, input = {}) => window.ipc.postMessage(JSON.stringify({
    version: 1, id: `native_${operation}`, operation, input,
  }));
  const until = expression => new Promise((resolve, reject) => {
    if (expression()) return resolve();
    const observer = new MutationObserver(() => {
      if (expression()) { observer.disconnect(); clearTimeout(timer); resolve(); }
    });
    const timer = setTimeout(() => { observer.disconnect(); reject(new Error('Native surface condition timed out')); }, 30000);
    observer.observe(document.body, { subtree: true, attributes: true, childList: true, characterData: true });
  });
  let nativeReady = false;
  window.chrome.webview.addEventListener('message', event => {
    if (event.data.event === 'native_console_ready') {
      nativeReady = true;
      document.body.dataset.nativeConsoleReady = 'true';
    }
  });
  void (async () => {
    const primary = document.getElementById('primary');
    primary.click();
    await until(() => !document.getElementById('location').hidden);
    primary.click();
    await until(() => !document.getElementById('runtime').hidden && !primary.disabled);
    primary.click();
    await until(() => !document.getElementById('configuration').hidden && !primary.disabled);
    primary.click();
    await until(() => !document.getElementById('folders').hidden);
    primary.click();
    await until(() => !document.getElementById('progress').hidden);
    document.getElementById('details').open = true;
    await until(() => nativeReady && document.getElementById('total').value >= 60);
    const placeholder = document.getElementById('console');
    if (placeholder.textContent || placeholder.tagName === 'PRE') throw new Error('Duplicate HTML console exists');
    for (const element of document.querySelectorAll('body, main, #progress, #details, #console')) {
      const style = getComputedStyle(element);
      if (['scroll', 'auto'].includes(style.overflowY) && element.scrollHeight > element.clientHeight + 1) {
        throw new Error(`Duplicate scrolling region: ${element.id || element.tagName}`);
      }
    }
    send('qualification_passed');
    send('close');
  })().catch(cause => { send('qualification_failed', String(cause)); send('close'); });
})();
