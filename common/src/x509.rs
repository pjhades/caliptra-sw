/*++

Licensed under the Apache-2.0 license.

File Name:

    x509.rs

Abstract:

    File contains X509 Certificate & CSR related utility functions

--*/

use caliptra_drivers::{okref, Ecc384PubKey, Sha256, Sha256Alg};
use caliptra_error::CaliptraResult;

/// X509 API
pub enum X509 {}

impl X509 {
    /// Get X509 Subject Serial Number
    ///
    /// # Arguments
    ///
    /// * `sha256`  - SHA-256 driver
    /// * `pub_key` - Public Key
    ///
    /// # Returns
    ///
    /// `[u8; 64]` - X509 Subject Identifier serial number
    pub fn subj_sn(sha256: &mut Sha256, pub_key: &Ecc384PubKey) -> CaliptraResult<[u8; 64]> {
        let data = pub_key.to_der();
        let digest = sha256.digest(&data);
        let digest = okref(&digest)?;
        Ok(Self::hex(&digest.into()))
    }

    /// Get Cert Subject Key Identifier
    ///
    /// # Arguments
    ///
    /// * `sha256`  - SHA-256 driver
    /// * `pub_key` - Public Key
    ///
    /// # Returns
    ///
    /// `[u8; 20]` - X509 Subject Key Identifier
    pub fn subj_key_id(sha256: &mut Sha256, pub_key: &Ecc384PubKey) -> CaliptraResult<[u8; 20]> {
        let data = pub_key.to_der();
        let digest = sha256.digest(&data);
        let digest: [u8; 32] = okref(&digest)?.into();
        Ok(digest[..20].try_into().unwrap())
    }

    /// Return the hex representation of the input `buf`
    ///
    /// # Arguments
    ///
    /// `buf` - Buffer
    ///
    /// # Returns
    ///
    /// `[u8; 64]` - Hex representation of the buffer
    fn hex(buf: &[u8; 32]) -> [u8; 64] {
        fn ch(byte: u8) -> u8 {
            match byte & 0x0F {
                b @ 0..=9 => 48 + b,
                b @ 10..=15 => 55 + b,
                _ => unreachable!(),
            }
        }

        let mut hex = [0u8; 64];

        for (index, byte) in buf.iter().enumerate() {
            hex[index << 1] = ch((byte & 0xF0) >> 4);
            hex[(index << 1) + 1] = ch(byte & 0x0F);
        }

        hex
    }
}
