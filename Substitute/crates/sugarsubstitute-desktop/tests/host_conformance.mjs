// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Run the SugarCubes host conformance kit against this host over the real page-channel codec.
//
// The page side is the kit itself, talking through the production bridge scripts in a VM. The
// native side is faked only at the binding: it returns credits and acknowledgements the way the
// desktop does, checks each framed page message against the host's recorded answers
// (`conformance_answers.json`, which a Windows test records from the production owners) and
// delivers those answers through the bridge's own delivery entry point. The kit's transcript is
// this host's SugarCubes profile; `--record` saves it, otherwise it must equal the saved one.
//
//   node --experimental-transform-types --no-warnings --import ./tests/typescript_resolution.mjs \
//     tests/host_conformance.mjs [--record]
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import test from 'node:test';
import { Script, createContext } from 'node:vm';

const sugar = new URL('../../../../Sugar/integrations/shared/conformance/', import.meta.url);
const { runHostConformance } = await import(new URL('HostConformance.ts', sugar).href);
const { recordHostProfile, replayHostProfile } = await import(new URL('HostProfiles.ts', sugar).href);
const panel = new URL('../src/main_actor/panel/', import.meta.url);
const answers = JSON.parse(readFileSync(
  new URL('../../sugarsubstitute/src/production/child/panel/conformance_answers.json', import.meta.url), 'utf8'));
const release = /^version = "([^"]+)"$/m.exec(readFileSync(new URL('../../../Cargo.toml', import.meta.url), 'utf8'))[1];
const profilePath = new URL(`profiles/substitute/${release}.json`, sugar);
const binding = '__sugarcubes_substitute_send_v3';
const host = 'ai.artificialsweetener.sugarsubstitute';
const normalize = value => JSON.parse(JSON.stringify(value));

/** Install the production bridge scripts for the recorded session, as the desktop composes them. */
function page(session, post) {
  const script = name => readFileSync(new URL(name, panel), 'utf8');
  const bulk = `${script('bridge_bulk.js').trimEnd()}(${JSON.stringify('/__sugarsubstitute/bulk/')});\n`;
  const window = {};
  window.top = window;
  const context = createContext({ window, URLSearchParams, TextEncoder, TextDecoder, AbortController,
    location: { origin: 'http://127.0.0.1:8188',
      search: `?sc-panel-host=${host}&sc-panel-session=${session}&sc-panel-transport=native` },
    [binding]: post, addEventListener: () => undefined });
  for (const source of [script('bridge_protocol.js'), bulk, script('bridge.js')]) new Script(source).runInContext(context);
  return context;
}

/** The native side: credit, acknowledgement and the recorded answer to each framed page message. */
function native() {
  const credit = { c: 0, o: 0 };
  let context = null;
  let next = 0;
  let failure = null;
  const stopped = new Set();
  const fail = error => { failure ??= error; for (const stop of stopped) stop(); };
  const grant = lane => { credit[lane] += 1; context.__sugarcubes_substitute_transport_v1('g', lane, credit[lane]); };
  const receive = framed => setImmediate(() => {
    try {
      const [, lane, ticket, body] = /^1:([co]):(\d+):(.*)$/s.exec(framed) ?? fail(new Error(`Unframed ${framed}`));
      const exchange = answers.exchanges[next++];
      assert.ok(exchange, `The kit sent more than the recorded page messages: ${body}`);
      assert.deepEqual(JSON.parse(body), exchange.page);
      for (const answer of exchange.host)
        assert.equal(context.__sugarcubes_substitute_deliver_v3(answer, lane === 'o' ? Number(ticket) : undefined), true);
      if (lane === 'c') context.__sugarcubes_substitute_transport_v1('a', lane, Number(ticket));
      grant(lane);
    } catch (error) { fail(error); }
  });
  return {
    start(page) { context = page; grant('c'); grant('o'); },
    receive,
    consumed: () => next,
    failure: () => failure,
    cancellation: {
      cancelled: () => failure !== null,
      subscribe(stop) { stopped.add(stop); return () => stopped.delete(stop); },
    },
  };
}

test('the kit passes over the real bridge and records this host profile', async () => {
  const side = native();
  const context = page(answers.session, side.receive);
  side.start(context);
  const bridge = context.sugarcubesPanelHost;
  const transport = {
    send: message => bridge.send(message),
    subscribe: receive => bridge.subscribe(value => receive(normalize(value))),
  };
  let report;
  try {
    report = await runHostConformance(transport, { session: answers.session, cancellation: side.cancellation });
  } finally {
    if (side.failure()) throw side.failure();
  }
  assert.equal(side.consumed(), answers.exchanges.length, 'the kit skipped recorded page messages');
  assert.deepEqual(Object.fromEntries(report.results.map(({ check, outcome }) => [check, outcome])), {
    'host-description': 'passed', 'panel-presentation': 'passed', 'canvas-discovery': 'passed',
    'canvas-step-down': 'passed', 'canvas-masks': 'not-offered', memory: 'passed', updates: 'passed',
    'capability.presentation.host-styles': 'not-offered',
    'capability.presentation.system-accent': 'passed', 'capability.output.host-surface': 'passed',
  });
  const profile = normalize(recordHostProfile('substitute', release, answers.session, report));
  await replayHostProfile(profile);
  if (process.argv.includes('--record')) writeFileSync(profilePath, `${JSON.stringify(profile, null, 2)}\n`);
  else assert.deepEqual(JSON.parse(readFileSync(profilePath, 'utf8')), profile);
});
