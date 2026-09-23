// Licensed under the Apache-2.0 license

use crate::common::{rom_for_fw_integration_tests, run_rt_test, RuntimeTestArgs};
use caliptra_api::SocManager;
use caliptra_builder::firmware::APP_WITH_UART_STASH_MEASUREMENT_REGISTERS;
use caliptra_common::{
    memory_layout::{ROM_ORG, ROM_SIZE, ROM_STACK_ORG, ROM_STACK_SIZE, STACK_ORG, STACK_SIZE},
    FMC_ORG, FMC_SIZE, RUNTIME_ORG, RUNTIME_SIZE,
};
use caliptra_drivers::soc_ifc::stash_measurement::{StashMeasurementData, DWORDS_PER_SLOT};
use caliptra_hw_model::{
    CaliptraHwVersion, CodeRange, DefaultHwModel, HwModel, ImageInfo, InitParams, StackInfo,
    StackRange,
};
use caliptra_runtime::RtBootStatus;
use zerocopy::IntoBytes;

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

    model.step_until(|m| {
        m.soc_ifc().cptra_boot_status().read() == u32::from(RtBootStatus::RtReadyForCommands)
    });

    assert!(model.soc_ifc().stash_bank_status().read().cptra_lock());
}

//#[test]
//fn test_stash_measurement() {
//    let image_options = ImageOptions::default();
//    let runtime_test_args = RuntimeTestArgs {
//        test_image_options: Some(image_options.clone()),
//        ..Default::default()
//    };
//    let mut model = run_rt_test(runtime_test_args);
//
//    model.step_until(|m| {
//        m.soc_ifc().cptra_boot_status().read() == u32::from(RtBootStatus::RtReadyForCommands)
//    });
//
//    let measurement = [1u8; 48];
//    let mut cmd = MailboxReq::StashMeasurement(StashMeasurementReq {
//        hdr: MailboxReqHeader { chksum: 0 },
//        metadata: [0u8; 4],
//        measurement,
//        context: [0u8; 48],
//        svn: 0,
//    });
//    cmd.populate_chksum().unwrap();
//
//    let resp = model
//        .mailbox_execute(
//            u32::from(CommandId::STASH_MEASUREMENT),
//            cmd.as_bytes().unwrap(),
//        )
//        .unwrap()
//        .expect("We should have received a response");
//
//    let resp_hdr: &StashMeasurementResp =
//        StashMeasurementResp::ref_from_bytes(resp.as_bytes()).unwrap();
//
//    assert_eq!(resp_hdr.dpe_result, 0);
//
//    // create a new fw image with the runtime replaced by the mbox responder
//
//    let updated_fw_image = caliptra_builder::build_and_sign_image(
//        &FMC_WITH_UART,
//        crate::test_update_reset::mbox_test_image(),
//        image_options,
//    )
//    .unwrap();
//
//    // trigger an update reset so we can use commands in mbox responder
//    model
//        .mailbox_execute(
//            u32::from(CommandId::FIRMWARE_LOAD),
//            &updated_fw_image.to_bytes().unwrap(),
//        )
//        .unwrap();
//
//    let rt_current_pcr_resp = model.mailbox_execute(0x1000_0001, &[]).unwrap().unwrap();
//    let rt_current_pcr: [u8; 48] = rt_current_pcr_resp.as_bytes().try_into().unwrap();
//
//    let cptra_config_init_vals_hash: [u8; 48] =
//        calculate_cptra_config_init_vals_hash(&mut model, &updated_fw_image);
//
//    // hash expected DPE measurements in order to check that stashed measurement was added to DPE
//    let mut hasher = Sha384::new();
//    hasher.update(rt_current_pcr);
//    hasher.update(cptra_config_init_vals_hash);
//    if model.subsystem_mode() {
//        let (somv_measurement, somo_measurement) = default_rt_test_soc_manifest_measurements(0);
//        hasher.update(somv_measurement);
//        hasher.update(somo_measurement);
//        let mut mcu_hasher = Sha384::new();
//        mcu_hasher.update(DEFAULT_MCU_FW);
//        hasher.update(mcu_hasher.finalize());
//        // MCU ROM stashes field_entropy_state measurement
//        let mut fe_hasher = Sha384::new();
//        fe_hasher.update(0u32.to_le_bytes());
//        hasher.update(fe_hasher.finalize());
//    }
//    hasher.update(measurement);
//    let expected_measurement_hash = hasher.finalize();
//
//    let dpe_measurement_hash = model.mailbox_execute(0x3000_0000, &[]).unwrap().unwrap();
//    assert_eq!(expected_measurement_hash.as_bytes(), dpe_measurement_hash);
//}
//
//#[test]
//fn test_pcr31_extended_upon_stash_measurement() {
//    fn run_sequence(stash_measurement: bool) -> [u8; 48] {
//        let image_options = ImageOptions::default();
//        let runtime_test_args = RuntimeTestArgs {
//            test_image_options: Some(image_options.clone()),
//            ..Default::default()
//        };
//        let mut model = run_rt_test(runtime_test_args);
//
//        // update reset to the real runtime image
//        let updated_fw_image = caliptra_builder::build_and_sign_image(
//            &FMC_WITH_UART,
//            &APP_WITH_UART,
//            image_options.clone(),
//        )
//        .unwrap()
//        .to_bytes()
//        .unwrap();
//        model
//            .mailbox_execute(u32::from(CommandId::FIRMWARE_LOAD), &updated_fw_image)
//            .unwrap();
//
//        if stash_measurement {
//            let mut cmd = MailboxReq::StashMeasurement(StashMeasurementReq {
//                hdr: MailboxReqHeader { chksum: 0 },
//                metadata: [0u8; 4],
//                measurement: [2u8; 48],
//                context: [0u8; 48],
//                svn: 0,
//            });
//            cmd.populate_chksum().unwrap();
//
//            let _ = model
//                .mailbox_execute(
//                    u32::from(CommandId::STASH_MEASUREMENT),
//                    cmd.as_bytes().unwrap(),
//                )
//                .unwrap()
//                .expect("We should have received a response");
//        }
//
//        // update reset back to mbox responder so we can read PCR31
//        let updated_fw_image = caliptra_builder::build_and_sign_image(
//            &FMC_WITH_UART,
//            crate::test_update_reset::mbox_test_image(),
//            image_options.clone(),
//        )
//        .unwrap()
//        .to_bytes()
//        .unwrap();
//        model
//            .mailbox_execute(u32::from(CommandId::FIRMWARE_LOAD), &updated_fw_image)
//            .unwrap();
//
//        let updated_fw_image = caliptra_builder::build_and_sign_image(
//            &FMC_WITH_UART,
//            crate::test_update_reset::mbox_test_image(),
//            image_options,
//        )
//        .unwrap()
//        .to_bytes()
//        .unwrap();
//        model
//            .mailbox_execute(u32::from(CommandId::FIRMWARE_LOAD), &updated_fw_image)
//            .unwrap();
//
//        let pcr_31_resp = model.mailbox_execute(0x5000_0000, &[]).unwrap().unwrap();
//        pcr_31_resp.as_bytes().try_into().unwrap()
//    }
//
//    assert_ne!(run_sequence(false), run_sequence(true));
//}
//
///// Metadata `[2, 0, 0, 0]` is reserved for the Caliptra-managed MCU RT DPE
///// context: it tags the measurement with the `MCFW` TCI type and re-points the
///// cached MCU RT context index. `STASH_MEASUREMENT` performs no authorization,
///// so it must reject the reserved ID rather than let the SoC forge the MCU RT
///// measurement. (`AUTHORIZE_AND_STASH` may still use it, since that path
///// verifies the measurement against the signed SoC manifest.)
//#[test]
//fn test_stash_measurement_reserved_mcu_fw_id_rejected() {
//    let runtime_test_args = RuntimeTestArgs {
//        test_image_options: Some(ImageOptions::default()),
//        ..Default::default()
//    };
//    let mut model = run_rt_test(runtime_test_args);
//
//    model.step_until(|m| {
//        m.soc_ifc().cptra_boot_status().read() == u32::from(RtBootStatus::RtReadyForCommands)
//    });
//
//    let mut cmd = MailboxReq::StashMeasurement(StashMeasurementReq {
//        hdr: MailboxReqHeader { chksum: 0 },
//        metadata: MCU_RT_RESERVED_FW_ID,
//        measurement: [0xAAu8; 48],
//        context: [0u8; 48],
//        svn: 0,
//    });
//    cmd.populate_chksum().unwrap();
//
//    let resp = model
//        .mailbox_execute(
//            u32::from(CommandId::STASH_MEASUREMENT),
//            cmd.as_bytes().unwrap(),
//        )
//        .unwrap_err();
//
//    assert_error(
//        &mut model,
//        CaliptraError::RUNTIME_STASH_MEASUREMENT_RESERVED_FW_ID,
//        resp,
//    );
//}
