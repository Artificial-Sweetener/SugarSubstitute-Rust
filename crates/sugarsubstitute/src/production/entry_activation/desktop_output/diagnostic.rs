// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode only complete closed preparation fields, discarding every other raw line.
use sugarsubstitute_contracts::{execution::TerminalKind, transcript::OutputStream};
#[path = "diagnostic/bulk.rs"]
pub(super) mod bulk;
#[path = "diagnostic/close.rs"]
pub(super) mod close;
#[path = "diagnostic/console.rs"]
mod console;
#[path = "diagnostic/consume.rs"]
mod consume;
#[path = "diagnostic/failure.rs"]
pub(super) mod failure;
use failure::parse as parse_failure;
#[path = "diagnostic/main.rs"]
mod main;
#[path = "diagnostic/main_retirement.rs"]
mod main_retirement;
#[path = "diagnostic/startup.rs"]
mod startup;
pub(super) use main::MainFailure;
#[cfg(feature = "k1-native-qualification")]
#[path = "diagnostic/qualification.rs"]
pub(super) mod qualification;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Phase {
    pub(super) channel_phase: &'static str,
    pub(super) channel_terminal: Option<TerminalKind>,
    pub(super) capture_input: &'static str,
    pub(super) capture_admitted: bool,
    pub(super) capture: bool,
    pub(super) registered: bool,
    pub(super) configuration: bool,
    pub(super) original_failure: Option<&'static str>,
    pub(super) capture_failure: &'static str,
    pub(super) failure_retained: bool,
}
/// Retain the first whitelisted desktop failure as inert, run-bound output metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Failure {
    pub(super) boundary: &'static str,
    pub(super) kind: &'static str,
    pub(super) startup: Option<startup::StartupFailureDetail>,
    pub(super) consume: Option<consume::ConsumeFailureDetail>,
    pub(super) console: Option<console::ConsoleFailureDetail>,
}
/// Closed wire progress cannot establish a backend grant or native readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Connection {
    /// The original bounded request was prepared.
    Queued,
    /// The same request was admitted to its native writer.
    Admitted,
    /// Its exact reply transferred to the semantic owner.
    ReplyReceived,
    /// The exact matching reply decoded a grant; this is inert presentation metadata.
    Granted,
    /// The exact matching reply decoded an original failure.
    Failed,
    /// The exact matching reply decoded an original rejection.
    Rejected,
}
/// Distinguish preparation progress from the first native failure without granting authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Observation {
    Preparation(Phase),
    Failure(Failure),
    Connection(Connection),
    Close(crate::close_diagnostic::CloseTrace),
    MainFailure(MainFailure),
    MainRetirement(main_retirement::MainRetirement),
    Transport(crate::production::child_transport_diagnostic::Record),
    Probe(sugarsubstitute_desktop::frontend_probe_diagnostic::Record),
    Readiness(crate::production::startup_readiness_diagnostic::Record),
    Canvas(crate::canvas_surface_diagnostic::Record),
    Bulk(sugarsubstitute_desktop::bulk_channel_diagnostic::Record),
    #[cfg(feature = "k1-native-qualification")]
    Qualification(qualification::Qualification),
}
/// Fixed source scratch is covered by the containing Record's original metadata
/// claim. Loss, oversized input and EOF invalidate partial lines; stdout is opaque.
pub(super) struct Diagnostic {
    bytes: [u8; 512],
    length: usize,
    discard: bool,
    sequences: [u64; 2],
    latest: Option<Phase>,
    connection: Option<Connection>,
    emitted: u8,
    failure_seen: bool,
    main_failure_seen: bool,
    retirements_emitted: u8,
    probe_emitted: u8,
    probe_timeout_seen: bool,
    canvas_emitted: u16,
    bulk_emitted: u16,
}
/// Canvas request lifecycle records relayed per child run; later records are discarded.
const CANVAS_RECORDS: u16 = 4096;
/// Bulk channel refusals relayed per child run; a page can provoke them, so later ones are
/// discarded.
const BULK_RECORDS: u16 = 256;
impl Diagnostic {
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; 512],
            length: 0,
            discard: false,
            sequences: [0; 2],
            latest: None,
            connection: None,
            emitted: 0,
            failure_seen: false,
            main_failure_seen: false,
            retirements_emitted: 0,
            probe_emitted: 0,
            probe_timeout_seen: false,
            canvas_emitted: 0,
            bulk_emitted: 0,
        }
    }
    /// The authenticated output owner validates Run before these inert byte facts.
    pub(super) fn feed(
        &mut self,
        stream: OutputStream,
        sequence: u64,
        gap: bool,
        end: bool,
        bytes: &[u8],
        mut emit: impl FnMut(Observation),
    ) {
        let at = stream.index();
        let discontinuity = gap || self.sequences[at].checked_add(1) != Some(sequence);
        if sequence <= self.sequences[at] {
            return;
        }
        self.sequences[at] = sequence;
        if stream != OutputStream::Stderr {
            return;
        }
        if discontinuity {
            self.length = 0;
            self.discard = true;
        }
        for &byte in bytes {
            if byte == b'\n' {
                if !self.discard {
                    #[cfg(feature = "k1-native-qualification")]
                    if let Some(record) = qualification::parse(&self.bytes[..self.length]) {
                        if self.emitted < 30 {
                            self.emitted += 1;
                            emit(Observation::Qualification(record));
                        }
                        self.length = 0;
                        self.discard = false;
                        continue;
                    }
                    if let Some(record) =
                        crate::canvas_surface_diagnostic::Record::parse(&self.bytes[..self.length])
                    {
                        // Canvas records keep their own budget, apart from the shared phase cap.
                        if self.canvas_emitted < CANVAS_RECORDS {
                            self.canvas_emitted += 1;
                            emit(Observation::Canvas(record));
                        }
                    } else if let Some(record) =
                        sugarsubstitute_desktop::bulk_channel_diagnostic::Record::parse(
                            &self.bytes[..self.length],
                        )
                    {
                        // Page-provoked refusals keep their own budget, apart from the phase cap.
                        if self.bulk_emitted < BULK_RECORDS {
                            self.bulk_emitted += 1;
                            emit(Observation::Bulk(record));
                        }
                    } else if let Some(probe) =
                        sugarsubstitute_desktop::frontend_probe_diagnostic::Record::parse(
                            &self.bytes[..self.length],
                        )
                    {
                        let timeout = probe.stage
                            == sugarsubstitute_desktop::frontend_probe_diagnostic::Stage::Timeout;
                        if timeout && !self.probe_timeout_seen {
                            self.probe_timeout_seen = true;
                            emit(Observation::Probe(probe));
                        } else if !timeout && self.probe_emitted < 16 {
                            self.probe_emitted += 1;
                            emit(Observation::Probe(probe));
                        }
                    } else if let Some(readiness) =
                        crate::production::startup_readiness_diagnostic::Record::parse(
                            &self.bytes[..self.length],
                        )
                    {
                        if self.emitted < 30 {
                            self.emitted += 1;
                            emit(Observation::Readiness(readiness));
                        }
                    } else if let Some(close) = close::parse(&self.bytes[..self.length]) {
                        if self.emitted < 30 {
                            self.emitted += 1;
                            emit(Observation::Close(close));
                        }
                    } else if let Some(retirement) =
                        main_retirement::parse(&self.bytes[..self.length])
                    {
                        if self.retirements_emitted < 16 {
                            self.retirements_emitted += 1;
                            emit(Observation::MainRetirement(retirement));
                        }
                    } else if let Some(failure) = main::parse(&self.bytes[..self.length]) {
                        if !self.main_failure_seen && self.emitted < 32 {
                            self.main_failure_seen = true;
                            self.emitted += 1;
                            emit(Observation::MainFailure(failure));
                        }
                    } else if let Some(failure) =
                        crate::production::child_transport_diagnostic::parse(
                            &self.bytes[..self.length],
                        )
                    {
                        if !self.failure_seen {
                            self.failure_seen = true;
                            if self.emitted < 32 && (self.main_failure_seen || self.emitted < 31) {
                                self.emitted += 1;
                                emit(Observation::Transport(failure));
                            }
                        }
                    } else if let Some(failure) = parse_failure(&self.bytes[..self.length]) {
                        if !self.failure_seen {
                            self.failure_seen = true;
                            if self.emitted < 32 && (self.main_failure_seen || self.emitted < 31) {
                                self.emitted += 1;
                                emit(Observation::Failure(failure));
                            }
                        }
                    } else if let Some(connection) = parse_connection(&self.bytes[..self.length]) {
                        let next = match self.connection {
                            None => Some(Connection::Queued),
                            Some(Connection::Queued) => Some(Connection::Admitted),
                            Some(Connection::Admitted) => Some(Connection::ReplyReceived),
                            Some(Connection::ReplyReceived) => match connection {
                                Connection::Granted | Connection::Failed | Connection::Rejected => {
                                    Some(connection)
                                }
                                _ => None,
                            },
                            Some(
                                Connection::Granted | Connection::Failed | Connection::Rejected,
                            ) => None,
                        };
                        if next == Some(connection) {
                            self.connection = Some(connection);
                            // All nonfailure observations share the original cap and reserve.
                            if self.emitted < 30 {
                                self.emitted += 1;
                                emit(Observation::Connection(connection));
                            }
                        }
                    } else if let Some(phase) = parse(&self.bytes[..self.length])
                        && self.latest != Some(phase)
                    {
                        self.latest = Some(phase);
                        // Reserve the final two unchanged32 slots for generic and main failures.
                        if self.emitted < 30 {
                            self.emitted += 1;
                            emit(Observation::Preparation(phase));
                        }
                    }
                }
                self.length = 0;
                self.discard = false;
            } else if !self.discard {
                if self.length == self.bytes.len() {
                    self.length = 0;
                    self.discard = true;
                } else {
                    self.bytes[self.length] = byte;
                    self.length += 1;
                }
            }
        }
        if end {
            self.length = 0;
            self.discard = false;
        }
    }
}
/// Accept one exact closed token, rejecting fields, duplicates and raw payload extensions.
fn parse_connection(bytes: &[u8]) -> Option<Connection> {
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"child_connection_transition\" ")?;
    if prefix.contains("operation=") {
        return None;
    }
    match fields {
        "stage=Queued" => Some(Connection::Queued),
        "stage=Admitted" => Some(Connection::Admitted),
        "stage=ReplyReceived" => Some(Connection::ReplyReceived),
        "stage=Granted" => Some(Connection::Granted),
        "stage=Failed" => Some(Connection::Failed),
        "stage=Rejected" => Some(Connection::Rejected),
        _ => None,
    }
}
fn parse(bytes: &[u8]) -> Option<Phase> {
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let marker = "operation=\"child_preparation_transition\" ";
    let (prefix, fields) = line.split_once(marker)?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let channel_phase = match fields.next()?.strip_prefix("channel_phase=")? {
        "Unadmitted" => "Unadmitted",
        "Admitted" => "Admitted",
        "Prepared" => "Prepared",
        "Connected" => "Connected",
        "Authenticated" => "Authenticated",
        _ => return None,
    };
    let channel_terminal = match fields.next()?.strip_prefix("channel_terminal=")? {
        "None" => None,
        "Some(Success)" => Some(TerminalKind::Success),
        "Some(Failure)" => Some(TerminalKind::Failure),
        "Some(Panicked)" => Some(TerminalKind::Panicked),
        "Some(Cancelled)" => Some(TerminalKind::Cancelled),
        "Some(TimedOut)" => Some(TerminalKind::TimedOut),
        "Some(Coalesced)" => Some(TerminalKind::Coalesced),
        _ => return None,
    };
    let capture_input = match fields.next()?.strip_prefix("capture_input=")? {
        "Unchecked" => "Unchecked",
        "NoPeer" => "NoPeer",
        "Absent" => "Absent",
        "Transferred" => "Transferred",
        "Closed" => "Closed",
        _ => return None,
    };
    let capture_admitted = boolean(fields.next()?, "capture_admitted=")?;
    let capture = boolean(fields.next()?, "capture=")?;
    let registered = boolean(fields.next()?, "registered=")?;
    let configuration = boolean(fields.next()?, "configuration=")?;
    let original_failure = match fields.next()?.strip_prefix("original_failure=")? {
        "None" => None,
        "Some(State)" => Some("State"),
        "Some(Identity)" => Some("Identity"),
        "Some(Transport)" => Some("Transport"),
        "Some(Capture)" => Some("Capture"),
        "Some(Configuration)" => Some("Configuration"),
        "Some(Deadline)" => Some("Deadline"),
        _ => return None,
    };
    let capture_failure = match fields.next()?.strip_prefix("capture_failure=")? {
        "None" => "None",
        "State" => "State",
        "Identity" => "Identity",
        "Admission" => "Admission",
        "Native" => "Native",
        "Data" => "Data",
        "Source" => "Source",
        "TimedOut" => "TimedOut",
        "Cancelled" => "Cancelled",
        "Uncertain" => "Uncertain",
        "Other" => "Other",
        _ => return None,
    };
    let failure_retained = boolean(fields.next()?, "failure_retained=")?;
    if fields.next().is_some() {
        return None;
    }
    Some(Phase {
        channel_phase,
        channel_terminal,
        capture_input,
        capture_admitted,
        capture,
        registered,
        configuration,
        original_failure,
        capture_failure,
        failure_retained,
    })
}

fn boolean(field: &str, prefix: &str) -> Option<bool> {
    match field.strip_prefix(prefix)? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}
#[cfg(test)]
#[path = "diagnostic/canvas_tests.rs"]
mod canvas_tests;
#[cfg(test)]
#[path = "diagnostic/readiness_tests.rs"]
mod readiness_tests;
#[cfg(test)]
#[path = "tests.rs"]
pub(super) mod tests;
