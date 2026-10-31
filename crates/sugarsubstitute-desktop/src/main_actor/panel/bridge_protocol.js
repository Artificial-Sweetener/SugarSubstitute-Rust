// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Match transport custody to the shared canvas protocol; payloads remain native-owned.
// The host invokes this expression with 'host' only while its main browser is composition-hosted.
((outputSurfacePreference) => {
  const host = 'ai.artificialsweetener.sugarsubstitute';
  const controls = ['ready', 'applied', 'pong', 'rejected'];
  const requestTypes = ['context.request', 'surface.capabilities.request', 'surface.viewport.request',
    'surface.presentation.request', 'surface.prepare', 'surface.geometry', 'surface.select',
    'surface.commit', 'surface.release', 'surface.preview.begin', 'surface.preview.chunk',
    'surface.preview.end', 'surface.preview.withdraw', 'surface.overview'];
  const policy = Object.freeze({
    host,
    outputSurfacePreference: outputSurfacePreference === 'host' ? 'host' : undefined,
    session(search) {
      const query = new URLSearchParams(search);
      const session = query.get('sc-panel-session');
      if (query.getAll('sc-panel-host').length !== 1 || query.get('sc-panel-host') !== host ||
          query.getAll('sc-panel-session').length !== 1 || !session ||
          !/^[a-zA-Z0-9-]{16,80}$/.test(session) ||
          query.getAll('sc-panel-transport').length !== 1 || query.get('sc-panel-transport') !== 'native') return null;
      return session;
    },
    control: type => controls.includes(type),
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
    acceptsReply(type, value) {
      const expected = type.startsWith('surface.preview.') ? 'surface.reply' :
        type.replace(/[^.]+$/, 'reply');
      return value.type === expected || value.type === 'error.reply' ||
        expected === 'surface.viewport.reply' && value.type === 'surface.reply' &&
        ['stale', 'cancelled', 'unavailable', 'error'].includes(value.status);
    },
    notification: value => value.protocol === 'sugarcubes.panel' ||
      value.protocol === 'sugarcubes.canvas' && value.requestId === 0 &&
      value.type === 'surface.presentation.changed',
  });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_policy_v3', { value: policy });
  globalThis.__sugarcubes_substitute_install_v3?.();
})
