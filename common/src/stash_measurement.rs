/*++

Licensed under the Apache-2.0 license.

File Name:

    stash_measurement.rs

Abstract:

    File contains STASH_MEASUREMENT mailbox command.

--*/

use crate::mailbox_api::{StashMeasurementData, StashMeasurementReq, StashMeasurementResp};
use crate::pcr::PCR_ID_STASH_MEASUREMENT;
use crate::{PcrLogEntry, PcrLogEntryId};
use caliptra_drivers::pcr_log::MeasurementLogEntry;
use caliptra_drivers::{CaliptraError, CaliptraResult, PcrBank, PersistentData, Sha2_512_384};
use zerocopy::{FromBytes, IntoBytes};

pub struct StashMeasurementCmd;
impl StashMeasurementCmd {
    #[inline(always)]
    pub(crate) fn execute(
        cmd_bytes: &[u8],
        pcr_bank: &mut PcrBank,
        sha2_512_384: &mut Sha2_512_384,
        persistent_data: &mut PersistentData,
        _resp: &mut [u8],
    ) -> CaliptraResult<usize> {
        let req = StashMeasurementReq::ref_from_bytes(cmd_bytes)
            .map_err(|_| CaliptraError::FW_PROC_MAILBOX_INVALID_REQUEST_LENGTH)?;

        // Extend measurement into PCR31.
        extend_measurement(pcr_bank, sha2_512_384, persistent_data, &req.data)?;

        // Zero value of response buffer is good
        Ok(core::mem::size_of::<StashMeasurementResp>())
    }
}

pub fn extend_measurement(
    pcr_bank: &mut PcrBank,
    sha2: &mut Sha2_512_384,
    persistent_data: &mut PersistentData,
    stash_measurement: &StashMeasurementData,
) -> CaliptraResult<()> {
    // Extend measurement into PCR31.
    pcr_bank.extend_pcr(
        PCR_ID_STASH_MEASUREMENT,
        sha2,
        stash_measurement.measurement.as_bytes(),
    )?;

    // Log measurement to the measurement log.
    log_measurement(persistent_data, stash_measurement)
}

/// Log measurement data to the Stash Measurement log
///
/// # Arguments
/// * `persistent_data` - Persistent data
/// * `stash_measurement` - Measurement
///
/// # Return Value
/// * `Ok(())` - Success
/// * `Err(GlobalErr::MeasurementLogExhausted)` - Measurement log exhausted
///
fn log_measurement(
    persistent_data: &mut PersistentData,
    stash_measurement: &StashMeasurementData,
) -> CaliptraResult<()> {
    let fht = &mut persistent_data.rom.fht;
    let Some(dst) = persistent_data
        .rom
        .measurement_log
        .get_mut(fht.meas_log_index as usize)
    else {
        return Err(CaliptraError::ROM_GLOBAL_MEASUREMENT_LOG_EXHAUSTED);
    };

    *dst = MeasurementLogEntry {
        pcr_entry: PcrLogEntry {
            id: PcrLogEntryId::StashMeasurement as u16,
            reserved0: [0u8; 2],
            pcr_ids: 1 << (PCR_ID_STASH_MEASUREMENT as u8),
            pcr_data: zerocopy::transmute!(stash_measurement.measurement),
        },
        metadata: stash_measurement.metadata,
        context: zerocopy::transmute!(stash_measurement.context),
        svn: stash_measurement.svn,
        reserved0: [0u8; 4],
    };

    fht.meas_log_index += 1;

    Ok(())
}
