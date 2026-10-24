// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Embedded only by the off-screen native qualification entry.
(() => {
  const send = (operation, input = {}) => window.ipc.postMessage(JSON.stringify({
    version: 1, id: `native_${operation}`, operation, input,
  }));
  const until = (expression, stage, milliseconds = 30000) => new Promise((resolve, reject) => {
    if (expression()) return resolve();
    const observer = new MutationObserver(() => {
      if (expression()) { observer.disconnect(); clearTimeout(timer); resolve(); }
    });
    const timer = setTimeout(() => { observer.disconnect(); reject(new Error(`Native surface condition timed out: ${stage}`)); }, milliseconds);
    observer.observe(document.body, { subtree: true, attributes: true, childList: true, characterData: true });
  });
  let nativeReady = false;
  let sourceReady = false;
  window.chrome.webview.addEventListener('message', event => {
    if (event.data.event === 'qualification_source_ready') {
      sourceReady = true;
      document.body.dataset.qualificationSourceReady = 'true';
    }
    if (event.data.event === 'native_console_ready') {
      nativeReady = true;
      document.body.dataset.nativeConsoleReady = 'true';
    }
  });
  void (async () => {
    await until(() => sourceReady, 'authenticated source readiness', 90000);
    for (const locale of window.setupBootstrap.locales) {
      const selector = document.getElementById('locale');
      selector.value = locale.id;
      selector.dispatchEvent(new Event('change'));
      await until(() => !selector.disabled && document.documentElement.lang === locale.id, 'localized native title');
      if (!document.querySelector('[data-text=languageTitle]').textContent)
        throw new Error('Selected installer locale has no language heading');
      if (document.querySelector('#brand svg desc').textContent.includes('__'))
        throw new Error('Selected installer locale has no wordmark description');
    }
    const primary = document.getElementById('primary');
    primary.click();
    await until(() => !document.getElementById('location').hidden, 'location');
    primary.click();
    await until(() => !document.getElementById('runtime').hidden && !primary.disabled, 'runtime');
    primary.click();
    await until(() => !document.getElementById('configuration').hidden && !primary.disabled, 'configuration');
    primary.click();
    await until(() => !document.getElementById('folders').hidden, 'folders');
    primary.click();
    await until(() => !document.getElementById('progress').hidden, 'progress');
    document.getElementById('details').open = true;
    await until(() => nativeReady && document.getElementById('total').value >= 60, 'native console');
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
