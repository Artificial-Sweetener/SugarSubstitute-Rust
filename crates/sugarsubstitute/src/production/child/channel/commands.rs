// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit bounded child commands into the actual transport's existing storage.
use super::*;

impl ChildChannel {
    pub(super) fn advance(&mut self) -> Result<()> {
        match self.exchange {
            Exchange::Preparing => {}
            Exchange::Welcome => {
                if self.read.is_none() && !self.hello_admitted {
                    self.admit_read(false)?;
                }
                if self.hello_pending && !self.hello_admitted {
                    let binding =
                        self.handshake.as_ref().ok_or("Lifecycle Hello binding missing")?;
                    let capability =
                        self.capability.as_ref().ok_or("Lifecycle Hello capability missing")?;
                    let message = WireMessage {
                        header: binding.hello_header()?,
                        payload: WirePayload::Hello(CapabilityRef::new(capability.bytes())),
                    };
                    let original =
                        self.original_deadline().ok_or("Lifecycle handshake cutoff missing")?;
                    if Self::admit_write(
                        self.session.as_ref().ok_or("Lifecycle session missing")?,
                        &mut self.latest,
                        &mut self.write,
                        &message,
                        original,
                    )? {
                        self.handshake
                            .as_mut()
                            .ok_or("Lifecycle Hello binding missing")?
                            .hello_admitted()?;
                        self.hello_admitted = true;
                        self.capability = None;
                    }
                }
            }
            Exchange::Register => {
                if !self.register_admitted {
                    let request = RequestId::new(1)?;
                    let header = self
                        .handshake
                        .as_ref()
                        .ok_or("Lifecycle Register binding missing")?
                        .register_header(request)?;
                    let message = WireMessage { header, payload: WirePayload::Register };
                    let original =
                        self.original_deadline().ok_or("Lifecycle handshake cutoff missing")?;
                    if Self::admit_write(
                        self.session.as_ref().ok_or("Lifecycle session missing")?,
                        &mut self.latest,
                        &mut self.write,
                        &message,
                        original,
                    )? {
                        self.handshake
                            .as_mut()
                            .ok_or("Lifecycle Register binding missing")?
                            .register_admitted(request)?;
                        self.register_admitted = true;
                    }
                }
                if self.register_admitted && self.read.is_none() {
                    self.admit_read(false)?;
                }
            }
            Exchange::Application => {
                self.advance_ui_response()?;
                if self.read.is_none() && self.received.is_none() {
                    self.admit_read(true)?;
                }
            }
        }
        Ok(())
    }

    fn admit_read(&mut self, passive: bool) -> Result<()> {
        let control = &self.session.as_ref().ok_or("Lifecycle read session missing")?.control;
        let next =
            self.latest.checked_add(1).ok_or("Lifecycle transport operation identity exhausted")?;
        let operation = LifecyclePipeOperationId::new(next)?;
        let result = if passive {
            control.read_passive(operation)
        } else {
            control.read(operation, control.original_deadline())
        };
        match result {
            Ok(()) => {
                self.latest = next;
                self.read = Some(PendingRead::new(operation));
            }
            Err(LifecyclePipeError::Busy) => {}
            Err(error) => return Err(Box::new(error)),
        }
        Ok(())
    }

    pub(super) fn admit_write(
        session: &LifecyclePipeSession,
        latest: &mut u64,
        pending: &mut Option<PendingWrite>,
        message: &WireMessage<'_>,
        original: Moment,
    ) -> Result<bool> {
        if pending.is_some() {
            return Ok(false);
        }
        let next =
            latest.checked_add(1).ok_or("Lifecycle transport operation identity exhausted")?;
        let operation = LifecyclePipeOperationId::new(next)?;
        let Some(mut buffer) = session.control.take_write_buffer()? else { return Ok(false) };
        let bytes = FrameCodec::encode(message, buffer.as_mut_bytes())?;
        match session.control.write(operation, original, buffer, bytes) {
            Ok(admission) => {
                drop(admission);
                *latest = next;
                *pending = Some(PendingWrite::new(operation, bytes));
                Ok(true)
            }
            Err(rejected) => {
                let error = rejected.error;
                drop(rejected.buffer);
                if error == LifecyclePipeError::Busy { Ok(false) } else { Err(Box::new(error)) }
            }
        }
    }
}
