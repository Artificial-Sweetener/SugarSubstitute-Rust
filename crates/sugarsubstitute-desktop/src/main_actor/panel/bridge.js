// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
(() => {
  if (window !== window.top) return;
  const query = new URLSearchParams(location.search);
  const host = 'ai.artificialsweetener.sugarsubstitute';
  const session = query.get('sc-panel-session');
  if (query.getAll('sc-panel-host').length !== 1 || query.get('sc-panel-host') !== host ||
      query.getAll('sc-panel-session').length !== 1 || !session ||
      !/^[a-zA-Z0-9-]{16,80}$/.test(session) ||
      query.getAll('sc-panel-transport').length !== 1 || query.get('sc-panel-transport') !== 'native') return;
  const binding = '__sugarcubes_substitute_send_v3';
  const sendNative = globalThis[binding];
  if (typeof sendNative !== 'function') return;
  const listeners = new Set();
  const encoder = new TextEncoder();
  const controls = ['ready', 'applied', 'pong', 'rejected'];
  const requestTypes = ['context.request', 'surface.capabilities.request', 'surface.prepare',
    'surface.geometry', 'surface.select', 'surface.commit', 'surface.release'];
  const lanes = Object.fromEntries(['o', 'c', 'x'].map(lane =>
    [lane, { credit: 0, last: 0, active: null }]));
  const queues = { o: [], c: [], x: [] };
  let closed = false, failed = false;
  function deny(code = 'unsupported', cause) {
    throw new TypeError('', { cause: { code: 'panel.transport.' + code, cause } });
  }
  function emit(value) {
    for (const listener of [...listeners]) if (listeners.has(listener)) listener(value);
  }
  function failure(record, status) {
    const value = record.value;
    emit({ protocol: value.protocol, version: 3, session, requestId: value.requestId,
      type: 'error.reply', failure: { status, message: '' } });
    record.done = true;
  }
  function queueControl(record, first = false) {
    const index = queues.c.findIndex(item => item.value.type === record.value.type);
    if (index >= 0) { if (!first) queues.c[index] = record; }
    else if (first) queues.c.unshift(record);
    else queues.c.push(record);
  }
  function flush(lane, direct = false) {
    const slot = lanes[lane];
    if (closed || failed || !slot.credit || slot.active) return true;
    const record = queues[lane].shift();
    if (!record) return true;
    record.done = false;
    record.ticket = slot.credit;
    slot.credit = 0;
    slot.active = record;
    try { sendNative('1:' + lane + ':' + record.ticket + ':' + record.encoded); }
    catch (error) {
      failed = true;
      if (lane === 'o' && !direct) failure(record, 'unavailable');
      if (direct) deny('binding', error);
      return false;
    }
    return true;
  }
  function cancel(record) {
    const id = record.value.requestId;
    const waiting = queues.o[0];
    if (waiting && waiting.value.requestId === id) {
      queues.o.length = 0;
      const value = waiting.value;
      if (value.type.startsWith('surface.') && value.type !== 'surface.capabilities.request')
        emit({ protocol: value.protocol, version: 3, session, requestId: id,
          type: 'surface.reply', address: value.address, revision: value.revision,
          status: 'cancelled', message: '' });
      else failure(waiting, 'cancelled');
      return;
    }
    const active = lanes.o.active;
    if (!active || active.done || active.value.requestId !== id) return;
    if (lanes.x.active?.encoded === record.encoded && !lanes.x.active.done) return;
    queues.x[0] = record;
    flush('x', true);
  }
  const bridge = Object.freeze({
    host, session, preferencePolicy: 'saved', outputSurfacePreference: 'host',
    send(input) {
      const encoded = JSON.stringify(input);
      if (typeof encoded !== 'string' || encoded.length > 16384 ||
          encoder.encode(encoded).byteLength > 16384)
        deny('bound');
      const envelope = JSON.stringify({ name: binding, payload: '1:o:9007199254740991:' + encoded, executionContextId: 0 });
      if (envelope.length + 20 > 16384 || encoder.encode(envelope).byteLength + 19 > 32768)
        deny('bound');
      const value = JSON.parse(encoded);
      if (!value || value.session !== session || value.version !== 3)
        deny();
      let lane;
      if (value.protocol === 'sugarcubes.panel' && controls.includes(value.type)) lane = 'c';
      else if (value.protocol === 'sugarcubes.canvas' && Number.isInteger(value.requestId) &&
          value.requestId >= 1 && value.requestId <= 2147483647) {
        if (value.type === 'cancel') lane = 'x';
        else if (requestTypes.includes(value.type)) lane = 'o';
      }
      if (!lane) deny();
      if (lane === 'x' && closed) return;
      if (closed || failed) deny('closed');
      const record = { encoded, value };
      if (lane === 'o') {
        if ([lanes.o.active, queues.o[0]].some(item => item && item.value.requestId === value.requestId))
          deny('duplicate');
        if (queues.o.length) deny('capacity');
        queues.o.push(record);
        flush('o', true);
      } else if (lane === 'c') {
        const active = lanes.c.active;
        if (active && !active.done && active.encoded === encoded) return;
        queueControl(record);
        flush('c', true);
      } else cancel(record);
    },
    subscribe(listener) {
      if (closed || typeof listener !== 'function' || listeners.size >= 8)
        deny('subscription');
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  Object.defineProperty(globalThis, 'sugarcubesPanelHost', { value: bridge });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_transport_v1', {
    value(action, lane, ticket) {
      if (closed || failed || !Object.hasOwn(lanes, lane) || !Number.isSafeInteger(ticket) || ticket < 1) return false;
      const slot = lanes[lane];
      if (action === 'g') {
        if (ticket <= slot.last || slot.credit || (slot.active && !slot.active.done)) return false;
        slot.active = null;
        slot.credit = ticket;
        slot.last = ticket;
        return flush(lane);
      }
      if (slot.credit === ticket && (action === 'r' || action === 'a' && lane !== 'o')) { slot.credit = 0; return true; }
      const active = slot.active;
      if (!active || active.ticket !== ticket || active.done) return false;
      if (action === 'a' && lane !== 'o') active.done = true;
      else if (action === 'r') {
        if (lane === 'o') { failure(active, 'unavailable'); queues.x.length = 0; }
        else {
          active.done = true;
          if (lane === 'c') queueControl(active, true);
          else if (lanes.o.active && !lanes.o.active.done &&
              lanes.o.active.value.requestId === active.value.requestId) queues.x[0] ??= active;
        }
      } else return false;
      return true;
    },
  });
  Object.defineProperty(globalThis, '__sugarcubes_substitute_deliver_v3', {
    value(value, ticket) {
      if (closed) return false;
      if (ticket === undefined) {
        if (!value || value.version !== 3 || value.protocol !== 'sugarcubes.panel' || value.session !== session) return false;
        emit(value);
        return true;
      }
      const active = lanes.o.active;
      if (!active || active.ticket !== ticket || active.done || !value || value.version !== 3 ||
          value.protocol !== 'sugarcubes.canvas' || value.session !== session ||
          value.requestId !== active.value.requestId) return false;
      const expected = active.value.type === 'context.request' ? 'context.reply' :
        active.value.type === 'surface.capabilities.request' ? 'surface.capabilities.reply' : 'surface.reply';
      if (value.type !== expected && value.type !== 'error.reply') return false;
      emit(value);
      active.done = true;
      if (queues.x[0]?.value.requestId === value.requestId) queues.x.length = 0;
      return true;
    },
  });
  addEventListener('pagehide', () => {
    closed = true;
    for (const queue of Object.values(queues)) queue.length = 0;
    listeners.clear();
  }, { once: true });
})();
