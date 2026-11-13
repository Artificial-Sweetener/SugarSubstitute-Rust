// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Match transport custody to the shared canvas protocol; payloads remain native-owned.
// What the host serves and declares travels in its host description, never on the bridge.
(() => {
  const host = 'ai.artificialsweetener.sugarsubstitute';
  const controls = ['ready', 'applied', 'pong', 'rejected'];
  const memoryReports = ['usage', 'preference'];
  const requestTypes = ['context.request', 'source.request', 'geometry.request', 'edit.request',
    'workflow.request', 'surface.capabilities.request', 'surface.viewport.request',
    'surface.presentation.request', 'surface.prepare', 'surface.geometry', 'surface.select',
    'surface.commit', 'surface.release', 'surface.preview.begin', 'surface.preview.chunk',
    'surface.preview.end', 'surface.preview.withdraw', 'surface.overview'];
  // The host description and memory budget protocols version independently of the version-three
  // transport.
  const version = value => ['sugarcubes.host', 'sugarcubes.memory'].includes(value.protocol) ? 1 : 3;
  const notification = value => value.protocol === 'sugarcubes.panel' ||
    value.protocol === 'sugarcubes.host' && value.type === 'description' ||
    value.protocol === 'sugarcubes.memory' && value.type === 'allowance' ||
    value.protocol === 'sugarcubes.canvas' && value.requestId === 0 &&
    ['surface.presentation.changed', 'surface.camera.changed', 'source.changed', 'edit.changed']
      .includes(value.type);
  function acceptsReply(type, value) {
    const expected = type.startsWith('surface.preview.') ? 'surface.reply' :
      type.replace(/[^.]+$/, 'reply');
    return value.type === expected || value.type === 'error.reply' ||
      expected === 'surface.viewport.reply' && value.type === 'surface.reply' &&
      ['stale', 'cancelled', 'unavailable', 'error'].includes(value.status);
  }
  const policy = Object.freeze({
    host,
    session(search) {
      const query = new URLSearchParams(search);
      const session = query.get('sc-panel-session');
      if (query.getAll('sc-panel-host').length !== 1 || query.get('sc-panel-host') !== host ||
          query.getAll('sc-panel-session').length !== 1 || !session ||
          !/^[a-zA-Z0-9-]{16,80}$/.test(session) ||
          query.getAll('sc-panel-transport').length !== 1 || query.get('sc-panel-transport') !== 'native') return null;
      return session;
    },
    control: value => value.protocol === 'sugarcubes.panel' && controls.includes(value.type) ||
      value.protocol === 'sugarcubes.memory' && memoryReports.includes(value.type),
    version,
    request: type => requestTypes.includes(type),
    cancelled(value) {
      const envelope = { protocol: value.protocol, version: 3, session: value.session,
        requestId: value.requestId };
      return value.type.startsWith('surface.') &&
        !['surface.capabilities.request', 'surface.presentation.request'].includes(value.type) ?
        { ...envelope, type: 'surface.reply', address: value.address, revision: value.revision,
          status: 'cancelled', message: '' } :
        { ...envelope, type: 'error.reply', failure: { status: 'cancelled', message: '' } };
    },
    // Settle a request the transport could not carry.
    failed: (value, status) => ({ protocol: value.protocol, version: 3, session: value.session,
      requestId: value.requestId, type: 'error.reply', failure: { status, message: '' } }),
    // Accept only the expected reply to one admitted request, inline or bulk.
    reply: (request, value) => !!value && value.version === 3 && value.protocol === 'sugarcubes.canvas' &&
      value.session === request.session && value.requestId === request.requestId &&
      acceptsReply(request.type, value),
    notification: (value, session) => !!value && value.version === version(value) &&
      value.session === session && notification(value),
  });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_policy_v3', { value: policy });
  globalThis.__sugarcubes_substitute_install_v3?.();
})();
