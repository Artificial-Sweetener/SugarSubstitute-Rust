//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove the page offers a host's update report only while the host says it has one. */
import { expect, test } from '@jest/globals';
import {
  HostUpdateReport,
  HostUpdateReportLink,
} from '../../../frontend/comfyui/ui/core/update_report/HostUpdateReportLink.js';

const session = 'session-0123456789abcdef';
const envelope = { protocol: 'sugarcubes.update-report', version: 1, session };

/** A scripted host channel. */
function channel() {
  const sent: unknown[] = [];
  let receive: (value: unknown) => void = () => undefined;
  let subscribed = false;
  return {
    sent,
    subscribed: () => subscribed,
    host: (value: unknown) => receive(value),
    transport: {
      session,
      send: (value: unknown) => void sent.push(value),
      subscribe: (listener: (value: unknown) => void) => {
        receive = listener;
        subscribed = true;
        return () => {
          subscribed = false;
          receive = () => undefined;
        };
      },
    },
  };
}

test('the link watches, offers the report the host has, and names its state when asking', () => {
  const host = channel();
  const offer = new HostUpdateReport();
  let changes = 0;
  offer.subscribe(() => (changes += 1));
  const link = new HostUpdateReportLink(host.transport, offer, (error) => {
    throw error;
  });
  link.start();
  expect(host.sent).toEqual([{ ...envelope, type: 'watch' }]);
  expect(offer.available).toBe(false);
  offer.open();
  expect(host.sent).toHaveLength(1);

  host.host({ ...envelope, type: 'state', revision: 0, available: false });
  expect([offer.available, changes]).toEqual([false, 0]);
  host.host({ ...envelope, type: 'state', revision: 3, available: true });
  expect([offer.available, changes]).toEqual([true, 1]);
  offer.open();
  expect(host.sent[1]).toEqual({ ...envelope, type: 'open', revision: 3 });

  // A repeated or older state, another session's and a malformed one change nothing.
  host.host({ ...envelope, type: 'state', revision: 2, available: false });
  host.host({
    ...envelope,
    session: 'session-ffffffffffffffff',
    type: 'state',
    revision: 9,
    available: false,
  });
  host.host({ ...envelope, type: 'state', revision: 9 });
  expect([offer.available, changes]).toEqual([true, 1]);

  host.host({ ...envelope, type: 'state', revision: 4, available: false });
  expect([offer.available, changes]).toEqual([false, 2]);
  offer.open();
  expect(host.sent).toHaveLength(2);
});

test('a retired link withdraws the offer and sends nothing more', () => {
  const host = channel();
  const offer = new HostUpdateReport();
  const link = new HostUpdateReportLink(host.transport, offer, (error) => {
    throw error;
  });
  link.start();
  host.host({ ...envelope, type: 'state', revision: 1, available: true });
  expect(offer.available).toBe(true);
  link.dispose();
  expect([offer.available, host.subscribed()]).toEqual([false, false]);
  offer.open();
  expect(host.sent).toHaveLength(1);
});

test('a channel that refuses the watch retires the link and is reported once', () => {
  const reported: unknown[] = [];
  const failure = new Error('closed');
  const offer = new HostUpdateReport();
  const link = new HostUpdateReportLink(
    {
      session,
      send: () => {
        throw failure;
      },
      subscribe: () => () => undefined,
    },
    offer,
    (error) => void reported.push(error),
  );
  link.start();
  expect(reported).toEqual([failure]);
  expect(offer.available).toBe(false);
});
