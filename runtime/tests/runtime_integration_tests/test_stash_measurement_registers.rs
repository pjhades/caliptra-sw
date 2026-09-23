// Licensed under the Apache-2.0 license

use crate::common::{rom_for_fw_integration_tests, run_rt_test, RuntimeTestArgs};
use caliptra_api::SocManager;
use caliptra_builder::firmware::APP_WITH_UART_STASH_MEASUREMENT_REGISTERS;
use caliptra_common::{
    mailbox_api::{
        CommandId, GetTaggedTciReq, GetTaggedTciResp, MailboxReq, MailboxReqHeader,
        QuotePcrsEcc384Req, QuotePcrsEcc384Resp, TagTciReq,
    },
    memory_layout::{ROM_ORG, ROM_SIZE, ROM_STACK_ORG, ROM_STACK_SIZE, STACK_ORG, STACK_SIZE},
    FMC_ORG, FMC_SIZE, RUNTIME_ORG, RUNTIME_SIZE,
};
use caliptra_drivers::soc_ifc::stash_measurement::{StashMeasurementData, DWORDS_PER_SLOT};
use caliptra_hw_model::{
    CaliptraHwVersion, CodeRange, DefaultHwModel, HwModel, ImageInfo, InitParams, StackInfo,
    StackRange,
};
use caliptra_runtime::RtBootStatus;
use sha2::{Digest, Sha384};
use zerocopy::{FromBytes, IntoBytes};

//use caliptra_builder::ImageOptions;
//    firmware::{APP_WITH_UART, FMC_WITH_UART},
//use caliptra_common::mailbox_api::{
//    ActivateFirmwareReq, CommandId, MailboxReq, MailboxReqHeader, StashMeasurementReq,
//    StashMeasurementResp,
//};
//use caliptra_error::CaliptraError;
//use sha2::{Digest, Sha384};
//use zerocopy::{FromBytes, IntoBytes};
//
//use crate::common::{
//    assert_error, calculate_cptra_config_init_vals_hash, default_rt_test_soc_manifest_measurements,
//    run_rt_test, RuntimeTestArgs, DEFAULT_MCU_FW,
//};
//
///// Firmware ID reserved for the Caliptra-managed MCU RT DPE context.
//const MCU_RT_RESERVED_FW_ID: [u8; 4] = ActivateFirmwareReq::MCU_IMAGE_ID.to_le_bytes();

fn run_model(subsystem_mode: bool) -> DefaultHwModel {
    let rom = rom_for_fw_integration_tests().unwrap();
    let image_info = vec![
        ImageInfo::with_name(
            StackRange::new(ROM_STACK_ORG + ROM_STACK_SIZE, ROM_STACK_ORG),
            CodeRange::new(ROM_ORG, ROM_ORG + ROM_SIZE),
            "caliptra-rom".to_owned(),
        ),
        ImageInfo::with_name(
            StackRange::new(STACK_ORG + STACK_SIZE, STACK_ORG),
            CodeRange::new(FMC_ORG, FMC_ORG + FMC_SIZE),
            "caliptra-fmc".to_owned(),
        ),
        ImageInfo::with_name(
            StackRange::new(STACK_ORG + STACK_SIZE, STACK_ORG),
            CodeRange::new(RUNTIME_ORG, RUNTIME_ORG + RUNTIME_SIZE),
            "caliptra-runtime".to_owned(),
        ),
    ];
    let runtime_test_args = RuntimeTestArgs {
        test_fwid: Some(&APP_WITH_UART_STASH_MEASUREMENT_REGISTERS),
        successful_reach_rt: false,
        init_params: Some(InitParams {
            hw_version: CaliptraHwVersion::V2_2,
            rom: &rom,
            stack_info: Some(StackInfo::new(image_info)),
            subsystem_mode,
            ..Default::default()
        }),
        ..Default::default()
    };

    run_rt_test(runtime_test_args)
}

#[test]
fn test_drain_stash_measurements() {
    let mut model = run_model(false);
    let measurements = [
        StashMeasurementData {
            metadata: [0xa1; 4],
            measurement: [0xb1; 48],
            context: [0xc1; 48],
            svn: 0xdeadbeef,
        },
        StashMeasurementData {
            metadata: [0xa2; 4],
            measurement: [0xb2; 48],
            context: [0xc2; 48],
            svn: 0xdeadc0de,
        },
    ];

    // SoC writes measurements.
    for (i, measurement) in measurements.iter().enumerate() {
        for (j, chunk) in measurement.as_bytes().chunks_exact(4).enumerate() {
            model
                .soc_ifc()
                .stash_bank_slot_data()
                .at(i * DWORDS_PER_SLOT + j)
                .write(|_| u32::from_le_bytes(chunk.try_into().unwrap()))
        }
        model
            .soc_ifc()
            .stash_bank_soc_lock()
            .write(|x| x.lock(1 << i));
    }

    // SoC writes end-of-stash.
    model
        .soc_ifc()
        .stash_end_stash()
        .write(|x| x.end_stash(true));

    // Fast-forward to the mailbox command loop.
    model.step_until(|m| {
        m.soc_ifc().cptra_boot_status().read() == u32::from(RtBootStatus::RtReadyForCommands)
    });

    // At this point Caliptra should have draind the measurements and
    // have locked the bank.
    assert!(model.soc_ifc().stash_bank_status().read().cptra_lock());

    // Verify the measurements actually landed in PCR31.
    let mut cmd = MailboxReq::QuotePcrsEcc384(QuotePcrsEcc384Req {
        hdr: MailboxReqHeader { chksum: 0 },
        nonce: [0u8; 32],
    });
    cmd.populate_chksum().unwrap();

    let resp = model
        .mailbox_execute(
            u32::from(CommandId::QUOTE_PCRS_ECC384),
            cmd.as_bytes().unwrap(),
        )
        .unwrap()
        .expect("We should have received a response");
    let resp = QuotePcrsEcc384Resp::read_from_bytes(resp.as_slice()).unwrap();

    let mut expected_pcr31 = [0u8; 48];
    for measurement in measurements.iter() {
        let mut hasher = Sha384::new();
        hasher.update(expected_pcr31);
        hasher.update(measurement.measurement);
        expected_pcr31.copy_from_slice(&hasher.finalize());
    }
    assert_eq!(resp.pcrs[31], expected_pcr31);

    // Verify DPE actually derived a context for the drained measurements.
    // The default context should be derived from the last measurement, and
    // its current TCI should match the test measurement.
    const TAG: u32 = 0xc01d_cafe;
    let mut cmd = MailboxReq::TagTci(TagTciReq {
        hdr: MailboxReqHeader { chksum: 0 },
        // Default context handle.
        handle: [0u8; 16],
        tag: TAG,
    });
    cmd.populate_chksum().unwrap();
    model
        .mailbox_execute(u32::from(CommandId::DPE_TAG_TCI), cmd.as_bytes().unwrap())
        .unwrap()
        .expect("We should have received a response");

    let mut cmd = MailboxReq::GetTaggedTci(GetTaggedTciReq {
        hdr: MailboxReqHeader { chksum: 0 },
        tag: TAG,
    });
    cmd.populate_chksum().unwrap();
    let resp = model
        .mailbox_execute(
            u32::from(CommandId::DPE_GET_TAGGED_TCI),
            cmd.as_bytes().unwrap(),
        )
        .unwrap()
        .expect("We should have received a response");
    let resp = GetTaggedTciResp::read_from_bytes(resp.as_slice()).unwrap();

    assert_eq!(resp.tci_current, measurements[1].measurement);
    assert_ne!(resp.tci_cumulative, resp.tci_current);
    assert_ne!(resp.tci_cumulative, [0u8; 48]);
}

// more tests
// 1. test timer fires if soc never writes end-of-stash
