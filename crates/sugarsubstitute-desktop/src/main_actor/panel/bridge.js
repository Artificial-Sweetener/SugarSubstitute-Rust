// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Expose the public panel transport only in the selected top-level navigation.
(() => {
  if (window !== window.top) return;
  const parameters = new URLSearchParams(location.search);
  const host = 'ai.artificialsweetener.sugarsubstitute';
  const session = parameters.get('sc-panel-session');
  if (
    parameters.getAll('sc-panel-host').length !== 1 ||
    parameters.get('sc-panel-host') !== host ||
    parameters.getAll('sc-panel-session').length !== 1 ||
    !session ||
    !/^[a-zA-Z0-9-]{16,80}$/.test(session) ||
    parameters.getAll('sc-panel-transport').length !== 1 ||
    parameters.get('sc-panel-transport') !== 'native'
  ) return;
  const binding = '__sugarcubes_substitute_send_v3';
  const sendNative = globalThis[binding];
  if (typeof sendNative !== 'function') return;
  const listeners = new Set();
  const bridge = Object.freeze({
    host,
    session,
    preferencePolicy: 'saved',
    outputSurfacePreference: 'host',
    send(value) {
      const encoded = JSON.stringify(value);
      if (typeof encoded !== 'string' || encoded.length > 16384 ||
          new TextEncoder().encode(encoded).byteLength > 16384)
        throw new TypeError('Panel message exceeds its transport bound.');
      const envelope = JSON.stringify({ name: binding, payload: encoded, executionContextId: 0 });
      // Reserve all 20 signed context-ID characters and the native UTF-16 terminator.
      if (envelope.length + 19 + 1 > 16384 ||
          new TextEncoder().encode(envelope).byteLength + 19 > 32768)
        throw new TypeError('Panel message exceeds its transport bound.');
      sendNative(encoded);
    },
    subscribe(listener) {
      if (typeof listener !== 'function' || listeners.size >= 8)
        throw new TypeError('Panel subscription is unavailable.');
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  Object.defineProperty(globalThis, 'sugarcubesPanelHost', {
    value: bridge, configurable: false, writable: false,
  });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_deliver_v3', {
    value(value) {
      for (const listener of listeners) listener(value);
      return true;
    },
    configurable: false, writable: false,
  });
  addEventListener('pagehide', () => listeners.clear(), { once: true });
})();
