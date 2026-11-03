// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Move oversized panel messages through the host-owned bulk origin as exact JSON text.
// Tokens are host-issued and single-use; the host validates every uploaded byte again.
(() => {
  const origin = 'https://sugarsubstitute-bulk.invalid/';
  const inline = 16384, max = 100663296;
  // Captured at document start, before page scripts can replace them.
  const fetchNative = globalThis.fetch, Abort = globalThis.AbortController;
  const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });
  const transfers = new Set();
  let closed = false;
  const token = value => typeof value === 'string' && /^[0-9a-f]{32}$/.test(value);
  // Hold each transfer's abort handle until its body settles so pagehide can stop it.
  function request(path, init, read) {
    if (closed || typeof fetchNative !== 'function' || typeof Abort !== 'function')
      return Promise.reject(new TypeError(''));
    const abort = new Abort();
    transfers.add(abort);
    return new Promise(resolve => resolve(Reflect.apply(fetchNative, globalThis, [origin + path,
      { ...init, signal: abort.signal, mode: 'cors', credentials: 'omit', cache: 'no-store',
        redirect: 'error', referrerPolicy: 'no-referrer' }])))
      .then(read).finally(() => transfers.delete(abort));
  }
  // Always settle with the lane envelope: after a failed upload the host refuses the credit.
  function upload(grant, encoded, bytes) {
    const frame = JSON.stringify({ transport: 'sugarsubstitute.bulk', version: 1, token: grant, bytes });
    if (!token(grant)) return Promise.resolve(frame);
    return request('u/' + grant, { method: 'POST', body: encoded }, () => frame).catch(() => frame);
  }
  function download(value) {
    return request('d/' + value.token, { method: 'GET' }, async response => {
      if (response.status !== 200) throw new TypeError('');
      const body = await response.arrayBuffer();
      if (body.byteLength !== value.bytes) throw new TypeError('');
      return JSON.parse(decoder.decode(body));
    });
  }
  const bulk = Object.freeze({
    max,
    // Zero means both native lane bounds admit the message inline; otherwise its UTF-8 size.
    size(encoded, binding) {
      const bytes = encoder.encode(encoded).byteLength;
      return bytes <= inline && JSON.stringify({ name: binding,
        payload: '1:o:9007199254740991:' + encoded, executionContextId: 0 }).length + 20 <= inline ? 0 : bytes;
    },
    envelope: value => !!value && typeof value === 'object' && Object.keys(value).length === 4 &&
      value.transport === 'sugarsubstitute.bulk' && value.version === 1 && token(value.token) &&
      Number.isSafeInteger(value.bytes) && value.bytes > 0 && value.bytes <= max,
    // Post the lane envelope once the upload settles, if the record still owns its credit.
    send(grant, record, live, post) {
      upload(grant, record.encoded, record.bytes).then(frame => { if (live()) post(frame); });
    },
    // Release deliveries in host order; a pending bulk entry holds back those behind it.
    order(emit) {
      const outbox = [];
      return Object.freeze({
        push(entry) {
          if (entry) outbox.push(entry);
          while (outbox[0] && outbox[0].value !== undefined) {
            const { value } = outbox.shift();
            if (value) emit(value);
          }
        },
        clear() { outbox.length = 0; },
      });
    },
    // Hold an ordered slot until the exact bytes arrive; accept validates them or falls back.
    receive(value, order, accept) {
      const entry = {};
      const settle = message => { entry.value = accept(message); order.push(); };
      order.push(entry);
      download(value).then(settle, () => settle());
    },
    close() {
      closed = true;
      for (const abort of transfers) abort.abort();
      transfers.clear();
    },
  });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_bulk_v1', { value: bulk });
  globalThis.__sugarcubes_substitute_install_v3?.();
})();
