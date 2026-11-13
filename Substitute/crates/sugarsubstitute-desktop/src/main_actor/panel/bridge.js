// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
(() => {
  let installed = false;
  function install() {
    const policy = globalThis.__sugarcubes_substitute_policy_v3;
    const bulk = globalThis.__sugarcubes_substitute_bulk_v1;
    if (installed || !policy || !bulk) return;
    const host = policy.host;
    const session = policy.session(location.search);
    if (window !== window.top || !session) return;
    const binding = '__sugarcubes_substitute_send_v3';
    const sendNative = globalThis[binding];
    if (typeof sendNative !== 'function') return;
    const listeners = new Set();
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
    const order = bulk.order(emit);
    function failure(record, status) {
      order.push({ value: policy.failed(record.value, status) });
      record.done = true;
    }
    function queueControl(record, first = false) {
      const index = queues.c.findIndex(item => item.value.type === record.value.type);
      if (index >= 0) { if (!first) queues.c[index] = record; }
      else if (first) queues.c.unshift(record);
      else queues.c.push(record);
    }
    function broken(record, lane, direct, error) {
      failed = true;
      if (lane === 'o' && !direct) failure(record, 'unavailable');
      for (const item of queues.o.splice(0)) failure(item, 'unavailable');
      if (direct) deny('binding', error);
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
      const post = payload => sendNative('1:' + lane + ':' + record.ticket + ':' + payload);
      if (record.bytes) bulk.send(slot.upload, record, () => !closed && !failed && slot.active === record &&
        !record.done, frame => { try { post(frame); } catch (error) { broken(record, lane); } });
      else try { post(record.encoded); }
      catch (error) { broken(record, lane, direct, error); return false; }
      return true;
    }
    function cancel(record) {
      const id = record.value.requestId;
      const index = queues.o.findIndex(item => item.value.requestId === id);
      if (index >= 0) {
        const [waiting] = queues.o.splice(index, 1);
        order.push({ value: policy.cancelled(waiting.value) });
        waiting.done = true;
        return;
      }
      const active = lanes.o.active;
      if (!active || active.done || active.value.requestId !== id) return;
      if (lanes.x.active?.encoded === record.encoded && !lanes.x.active.done) return;
      queues.x[0] = record;
      flush('x', true);
    }
    const bridge = Object.freeze({
      host, session, preferencePolicy: 'saved',
      ...(policy.outputSurfacePreference === 'host' ? { outputSurfacePreference: 'host' } : {}),
      send(input) {
        const encoded = JSON.stringify(input);
        if (typeof encoded !== 'string') deny('bound');
        const bytes = bulk.size(encoded, binding);
        if (bytes > bulk.max) deny('bound');
        const value = JSON.parse(encoded);
        if (!value || value.session !== session || value.version !== policy.version(value))
          deny();
        let lane;
        if (policy.control(value)) lane = 'c';
        else if (value.protocol === 'sugarcubes.canvas' && Number.isInteger(value.requestId) &&
            value.requestId >= 1 && value.requestId <= 2147483647) {
          if (value.type === 'cancel') lane = 'x';
          else if (policy.request(value.type)) lane = 'o';
        }
        if (!lane) deny();
        if (bytes && lane !== 'o') deny('bound');
        if (lane === 'x' && closed) return;
        if (closed || failed) deny('closed');
        const record = { encoded, value, bytes };
        if (lane === 'o') {
          if ([lanes.o.active, ...queues.o].some(item => item && item.value.requestId === value.requestId))
            deny('duplicate');
          // Native credit admits one request at a time; this bounds page-side waiting work.
          if (queues.o.length >= 8) deny('capacity');
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
    installed = true;
    Object.defineProperty(globalThis, 'sugarcubesPanelHost', { value: bridge });
    Object.defineProperty(globalThis, '__sugarcubes_substitute_transport_v1', {
      value(action, lane, ticket, upload) {
        if (closed || failed || !Object.hasOwn(lanes, lane) || !Number.isSafeInteger(ticket) || ticket < 1) return false;
        const slot = lanes[lane];
        if (action === 'g') {
          if (ticket <= slot.last || slot.credit || (slot.active && !slot.active.done)) return false;
          slot.active = null;
          slot.credit = ticket;
          slot.last = ticket;
          slot.upload = upload;
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
        const big = bulk.envelope(value);
        let record = null;
        if (ticket === undefined) {
          if (!big && !policy.notification(value, session)) return false;
        } else {
          record = lanes.o.active;
          if (!record || record.ticket !== ticket || record.done || !big && !policy.reply(record.value, value))
            return false;
        }
        if (!big) order.push({ value });
        else bulk.receive(value, order, message => !record ? policy.notification(message, session) ? message : null :
          policy.reply(record.value, message) ? message : policy.failed(record.value, 'unavailable'));
        if (record) {
          record.done = true;
          if (queues.x[0]?.value.requestId === record.value.requestId) queues.x.length = 0;
        }
        return true;
      },
    });
    addEventListener('pagehide', () => {
      closed = true;
      bulk.close();
      for (const queue of Object.values(queues)) queue.length = 0;
      order.clear();
      listeners.clear();
    }, { once: true });
  }
  Object.defineProperty(globalThis, '__sugarcubes_substitute_install_v3', { value: install });
  install();
})();
