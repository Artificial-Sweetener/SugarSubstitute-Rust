// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit native-owned metadata streams inside the original exclusively retained generated grant.
use super::*;
use crate::durable_storage::streamed_record::{StreamRead, StreamStage, StreamWrite};

impl CacheLease {
    /// Keep the original grant while the shared storage owner opens an actual bounded record.
    pub(in crate::setup_storage) fn generated_read(
        &self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<StreamRead> {
        self.generated_record(context)?;
        StreamRead::open(&self._data, name, self.record.reserved, context)
    }
    /// Admit original bytes plus complete replacement scratch and its exact name before effects.
    pub(in crate::setup_storage) fn generated_write(
        &self,
        name: &str,
        length: u64,
        context: &JobContext,
    ) -> io::Result<StreamWrite> {
        self.generated_record(context)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let inventory = tree::generated_records(&self.directory, context)?;
        if inventory.bytes.checked_add(length).is_none_or(|bytes| bytes > self.record.reserved) {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Metadata byte grant exhausted",
            ));
        }
        let stage = StreamStage::plan(&self._data, name, length, context)?;
        inventory.admit_record_stage(stage.stage_name()?)?;
        stage.create(context)
    }
    /// Generated-record authority requires this owner's original writable, unsettled producer.
    fn generated_record(&self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.require_writable()?;
        if self.record.producer.is_none() {
            return Err(invalid("Metadata producer has retired"));
        }
        Ok(())
    }
}
