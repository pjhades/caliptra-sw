/*++

Licensed under the Apache-2.0 license.

File Name:

    x509.rs

Abstract:

    File contains X509 Certificate & CSR related utility functions

--*/
use crate::fmc_env::FmcEnv;
use caliptra_drivers::*;

/// X509 API
pub enum X509 {}

impl X509 {
    /// Get device serial number
    ///
    /// # Arguments
    ///
    /// * `env` - ROM Environment
    ///
    /// # Returns
    ///
    /// `[u8; 17]` - Byte 0 - Ueid Type, Bytes 1-16 Unique Endpoint Identifier
    pub fn ueid(env: &FmcEnv) -> CaliptraResult<[u8; 17]> {
        let ueid = env.soc_ifc.fuse_bank().ueid();
        Ok(ueid)
    }
}
