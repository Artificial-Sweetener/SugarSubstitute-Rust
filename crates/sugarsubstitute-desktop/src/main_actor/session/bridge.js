// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
(bootstrap => {
  if (window !== window.top || location.origin !== bootstrap.origin) return;
  const webview = bootstrap.webview;
  const scope = Object.freeze(bootstrap.scope);
  const listeners = new Set(), pending = new Map();
  const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
  const encoder = new TextEncoder();
  let next = 0, closed = false;
  function failure(code) { return new TypeError('', { cause: { code: 'session.transport.' + code } }); }
  function owns(message) {
    return message && message.protocol === 'sugarcubes.desktop-session' && message.version === 1 &&
      message.origin === bootstrap.origin && message.scope &&
      ['installation', 'profile', 'workspace', 'backend'].every(key => message.scope[key] === scope[key]);
  }
  function binding(value) {
    const keys = ['entryId', 'runtimeSession', 'generation', 'graphRevision', 'sourceRevision', 'navigationRevision'];
    return value && Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.includes(key)) &&
      uuid.test(value.entryId) && typeof value.runtimeSession === 'string' && value.runtimeSession.length > 0 &&
      value.runtimeSession.length <= 256 && keys.slice(2).every(key => Number.isSafeInteger(value[key]) && value[key] >= 0);
  }
  const host = {
    scope, origin: bootstrap.origin, webview,
    send(input) {
      if (closed) return Promise.reject(failure('closed'));
      if (pending.size >= 8) return Promise.reject(failure('capacity'));
      let message;
      try {
        const encoded = JSON.stringify(input);
        if (typeof encoded !== 'string' || encoder.encode(encoded).length > 524288) throw failure('bound');
        message = JSON.parse(encoded);
        if (!owns(message) || !uuid.test(message.requestId)) throw failure('scope');
      } catch (cause) { return Promise.reject(cause); }
      if (next >= Number.MAX_SAFE_INTEGER) return Promise.reject(failure('exhausted'));
      const ticket = ++next;
      return new Promise((resolve, reject) => {
        pending.set(ticket, { resolve, reject });
        try { chrome.webview.postMessage({ sessionTransport: 1, webview, ticket, message }); }
        catch (cause) { pending.delete(ticket); reject(failure('post')); }
      });
    },
    subscribe(listener) {
      if (closed || typeof listener !== 'function' || listeners.size >= 8) throw failure('subscription');
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  if (bootstrap.mediaPath) host.media = Object.freeze({
    version: 1,
    assetUrl(outputId, value) {
      if (closed || !uuid.test(outputId) || !binding(value)) return null;
      const exact = Object.fromEntries(['entryId', 'runtimeSession', 'generation', 'graphRevision', 'sourceRevision', 'navigationRevision'].map(key => [key, value[key]]));
      const url = bootstrap.origin + bootstrap.mediaPath + outputId + '?binding=' + encodeURIComponent(JSON.stringify(exact));
      return url.length <= 4096 ? url : null;
    },
  });
  const receive = event => {
    const value = event.data;
    if (closed || !value || value.sessionTransport !== 1 || value.webview !== webview) return;
    if (Object.hasOwn(value, 'ticket')) {
      const record = pending.get(value.ticket);
      if (!record) return;
      pending.delete(value.ticket);
      if (value.accepted === true) record.resolve();
      else record.reject(failure('refused'));
    } else if (owns(value.message)) {
      for (const listener of [...listeners]) if (listeners.has(listener)) listener(value.message);
    }
  };
  chrome.webview.addEventListener('message', receive);
  Object.defineProperty(window, 'sugarcubesSessionHost', { value: Object.freeze(host) });
  addEventListener('pagehide', () => {
    closed = true;
    chrome.webview.removeEventListener('message', receive);
    for (const record of pending.values()) record.reject(failure('closed'));
    pending.clear(); listeners.clear();
  }, { once: true });
})
