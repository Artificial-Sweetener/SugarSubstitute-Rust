// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// The creating-STA caller supplies only the sealed origin, PNG identity and remaining cutoff.
const abort = new AbortController();
let timer;
try {
  return await Promise.race([
    new Promise((_, reject) => {
      timer = setTimeout(() => {
        abort.abort();
        reject(new Error('download qualification original cutoff'));
      }, input.remaining_ms);
    }),
    (async () => {
      if (location.origin !== input.origin) throw new Error('download qualification origin');
      const app = window.comfyAPI?.app?.app;
      const manager = app?.extensionManager;
      if (!app?.isGraphReady || manager?.setting?.get('Comfy.PromptFilename') !== false)
        throw new Error('download qualification export needs prompt or app is unavailable');
      const command = manager.command;
      if (typeof command?.execute !== 'function' ||
          !command.commands?.some(value => value.id === 'Comfy.ExportWorkflow'))
        throw new Error('download qualification registered export unavailable');
      const modules = [...new Set(performance.getEntriesByType('resource')
        .map(entry => new URL(entry.name, location.href))
        .filter(url => url.origin === location.origin &&
          /^\/assets\/downloadUtil-[A-Za-z0-9_-]+\.js$/.test(url.pathname) && !url.search && !url.hash)
        .map(url => url.href))];
      if (modules.length !== 1) throw new Error('download qualification installed download module');
      const module = await import(modules[0]);
      const exported = name => {
        const found = Object.values(module).filter(value => typeof value === 'function' && value.name === name);
        if (found.length !== 1) throw new Error('download qualification installed export changed');
        return found[0];
      };
      const downloadFile = exported('downloadFile');
      const downloadBlob = exported('downloadBlob');
      const source = new URL('/view', location.origin);
      source.searchParams.set('filename', input.name);
      source.searchParams.set('type', 'output');
      const response = await fetch(source, { signal: abort.signal, cache: 'no-store' });
      if (!response.ok || !response.body || !response.headers.get('Content-Type')?.startsWith('image/png'))
        throw new Error('download qualification PNG response');
      const reader = response.body.getReader();
      const bytes = new Uint8Array(input.bytes);
      let offset = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          if (offset + value.length > bytes.length) throw new Error('download qualification PNG bound');
          bytes.set(value, offset);
          offset += value.length;
        }
      } finally {
        await reader.cancel();
        reader.releaseLock();
      }
      if (offset !== bytes.length ||
          ![137,80,78,71,13,10,26,10].every((byte, index) => bytes[index] === byte))
        throw new Error('download qualification PNG identity');
      const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map(byte => byte.toString(16).padStart(2, '0')).join('');
      if (digest !== input.sha256) throw new Error('download qualification PNG changed');
      abort.signal.throwIfAborted();
      await command.execute('Comfy.ExportWorkflow', { errorHandler: error => { throw error; } });
      abort.signal.throwIfAborted();
      downloadFile(source.href, 'k1-media-direct.png');
      downloadBlob('k1-media-blob.png', new Blob([bytes], { type: 'image/png' }));
      return JSON.stringify({schema:1, requests:3, png_bytes:offset, png_sha256:digest});
    })()
  ]);
} finally {
  clearTimeout(timer);
  abort.abort();
}
