/*++

Licensed under the Apache-2.0 license.

File Name:

    mailbox.rs

Abstract:

    File contains mailbox interface.

--*/

use core::mem::{align_of, size_of};
use core::slice;

use caliptra_drivers::{memory_layout, CaliptraResult};
use caliptra_error::CaliptraError;
use caliptra_registers::mbox::{
    enums::{MboxFsmE, MboxStatusE},
    MboxCsr,
};
use dpe::response::{DpeErrorCode, RespBufRead, RespBufWrite};
use zerocopy::{FromBytes, IntoBytes, Unalign};

use crate::CommandId;

pub struct Mailbox {
    mbox: MboxCsr,
}

impl Mailbox {
    /// Create a new Mailbox
    pub fn new(mbox: MboxCsr) -> Self {
        Self { mbox }
    }

    /// Check if there is a new command to be executed
    pub fn is_cmd_ready(&self) -> bool {
        let mbox = self.mbox.regs();
        mbox.status().read().mbox_fsm_ps().mbox_execute_uc()
    }

    /// Check if we are currently executing a mailbox command
    pub fn cmd_busy(&self) -> bool {
        let mbox = self.mbox.regs();
        mbox.status().read().status().cmd_busy()
    }

    /// Get the current state of the mailbox FSM
    pub fn mailbox_state(&self) -> MboxFsmE {
        let mbox = self.mbox.regs();
        mbox.status().read().mbox_fsm_ps()
    }

    /// Get the length of the current mailbox data in bytes
    pub fn dlen(&self) -> u32 {
        let mbox = self.mbox.regs();
        mbox.dlen().read()
    }

    /// Set the length of the current mailbox data in bytes
    pub fn set_dlen(&mut self, len: u32) -> CaliptraResult<()> {
        if len > memory_layout::MBOX_SIZE {
            return Err(CaliptraError::RUNTIME_MAILBOX_INVALID_PARAMS);
        }

        let mbox = self.mbox.regs_mut();
        mbox.dlen().write(|_| len);
        Ok(())
    }

    /// Get the length of the current mailbox data in words
    pub fn dlen_words(&self) -> u32 {
        self.dlen().div_ceil(4)
    }

    /// Get the CommandId from the mailbox
    pub fn cmd(&self) -> CommandId {
        let mbox = self.mbox.regs();
        let cmd_code = mbox.cmd().read();

        CommandId(cmd_code)
    }

    /// Lock the mailbox
    pub fn lock(&mut self) -> bool {
        let mbox = self.mbox.regs();
        mbox.lock().read().lock()
    }

    /// Unlock the mailbox
    pub fn unlock(&mut self) {
        let mbox = self.mbox.regs_mut();
        mbox.unlock().write(|_| 1.into());
    }

    /// Writes command `cmd` ot the mailbox if it is ready for a command
    pub fn write_cmd(&mut self, cmd: u32) -> CaliptraResult<()> {
        let mbox = self.mbox.regs_mut();
        match mbox.status().read().mbox_fsm_ps() {
            MboxFsmE::MboxRdyForCmd => {
                mbox.cmd().write(|_| cmd);
                Ok(())
            }
            _ => Err(CaliptraError::RUNTIME_INTERNAL),
        }
    }

    /// Gets the user of the mailbox
    pub fn user(&self) -> u32 {
        let mbox = self.mbox.regs();
        mbox.user().read()
    }

    /// Copies data in mailbox to `buf`
    pub fn copy_from_mbox(&mut self, buf: &mut [u32]) {
        let mbox = self.mbox.regs_mut();
        for word in buf {
            *word = mbox.dataout().read();
        }
    }

    /// Reads `word_count` words from the FIFO into `buf` as little-endian bytes.
    /// Handles structs whose size is not a multiple of 4.
    pub fn copy_from_mbox_bytes(&mut self, buf: &mut [u8], word_count: usize) {
        let mbox = self.mbox.regs_mut();
        for i in 0..word_count {
            let word_bytes = mbox.dataout().read().to_le_bytes();
            let offset = i * 4;
            if let Some(dst) = buf.get_mut(offset..) {
                let len = dst.len().min(4);
                dst[..len].copy_from_slice(&word_bytes[..len]);
            }
        }
    }

    /// Clears the mailbox
    pub fn flush(&mut self) {
        let count = self.dlen_words();
        let mbox = self.mbox.regs_mut();
        for _ii in 0..count {
            let _ = mbox.dataout().read();
        }
    }

    /// Copies `buf` to the mailbox
    pub fn copy_words_to_mbox(&mut self, buf: &[Unalign<u32>]) {
        let mbox = self.mbox.regs_mut();
        for word in buf {
            mbox.datain().write(|_| word.get());
        }
    }

    /// Copies word-aligned `buf` to the mailbox
    pub fn copy_bytes_to_mbox(&mut self, buf: &[u8]) -> CaliptraResult<()> {
        let count = buf.len() / size_of::<u32>();
        let (buf_words, suffix) = <[Unalign<u32>]>::ref_from_prefix_with_elems(buf, count).unwrap();
        self.copy_words_to_mbox(buf_words);
        if !suffix.is_empty() && suffix.len() <= size_of::<u32>() {
            let mut last_word = 0_u32;
            last_word.as_mut_bytes()[..suffix.len()].copy_from_slice(suffix);
            self.copy_words_to_mbox(&[Unalign::new(last_word)]);
        }
        Ok(())
    }

    /// Write a word-aligned `buf` to the mailbox
    pub fn write_response(&mut self, buf: &[u8]) -> CaliptraResult<()> {
        self.set_dlen(buf.len() as u32)?;
        self.copy_bytes_to_mbox(buf)?;
        Ok(())
    }

    /// Set mailbox status to `status`
    pub fn set_status(&mut self, status: MboxStatusE) {
        let mbox = self.mbox.regs_mut();
        mbox.status().write(|w| w.status(|_| status));
    }

    /// Retrieve a slice with the contents of the mailbox
    pub fn raw_mailbox_contents(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                memory_layout::MBOX_ORG as *const u8,
                memory_layout::MBOX_SIZE as usize,
            )
        }
    }

    /// Retrieve a mutable abstraction of the mailbox SRAM.
    pub fn mailbox_ram(&mut self) -> CaliptraResult<MailboxRam<'_>> {
        if !self.cmd_busy() {
            return Err(CaliptraError::DRIVER_MAILBOX_INVALID_STATE);
        }

        // SAFETY: At this point we know that we're currently processing a mailbox command so that
        // we have exclusive access to the underlying memory.
        let mem = unsafe {
            slice::from_raw_parts_mut(
                memory_layout::MBOX_ORG as *mut u32,
                memory_layout::MBOX_SIZE as usize,
            )
        };

        Ok(MailboxRam { mem })
    }
}

pub struct MailboxRam<'a> {
    mem: &'a mut [u32],
}

impl MailboxRam<'_> {
    const WORD_ALIGN: usize = align_of::<u32>();
    const WORD_SIZE: usize = size_of::<u32>();

    pub fn byte_len(&self) -> usize {
        self.mem.len() * Self::WORD_ALIGN
    }
}

impl RespBufRead for MailboxRam<'_> {
    fn read_at(&self, data: &mut [u8], offset: usize) -> Result<(), DpeErrorCode> {
        if offset > self.byte_len() || self.byte_len() - offset < data.len() {
            return Err(DpeErrorCode::InvalidResponseBuf);
        }

        let mut bytes_read = 0;
        let aligned_offset = offset.next_multiple_of(Self::WORD_ALIGN);
        let aligned_word_idx = aligned_offset / Self::WORD_SIZE;

        if aligned_offset != offset {
            let word_idx = aligned_word_idx - 1;
            let byte_idx = offset - word_idx * Self::WORD_SIZE;
            let word = self.mem[word_idx];
            let size = Self::WORD_SIZE - byte_idx;

            data.get_mut(..size)
                .ok_or(DpeErrorCode::InvalidResponseBuf)?
                .copy_from_slice(&word.to_le_bytes()[byte_idx..]);

            bytes_read += size;
        }

        let mut word_idx = aligned_word_idx;
        while bytes_read + Self::WORD_SIZE < data.len() {
            let word = self.mem[word_idx];
            data.get_mut(bytes_read..bytes_read + Self::WORD_SIZE)
                .ok_or(DpeErrorCode::InvalidResponseBuf)?
                .copy_from_slice(word.to_le_bytes().as_slice());
            bytes_read += Self::WORD_SIZE;
            word_idx += 1;
        }

        let remaining_size = data.len() - bytes_read;
        if remaining_size > 0 {
            let word = self.mem[word_idx];
            data.get_mut(bytes_read..bytes_read + remaining_size)
                .ok_or(DpeErrorCode::InvalidResponseBuf)?
                .copy_from_slice(&word.to_le_bytes()[..remaining_size]);
        }

        Ok(())
    }
}

impl RespBufWrite for MailboxRam<'_> {
    fn write_at(&mut self, data: &[u8], offset: usize) -> Result<(), DpeErrorCode> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MailboxRam;
    use dpe::response::{DpeErrorCode, RespBufRead, RespBufWrite};
    use zerocopy::transmute;

    #[test]
    fn test_respbuf_read() {
        let mut mem = [0xdeadbeef, 0xfeedface, 0xabadcafe];
        let sram = MailboxRam {
            mem: mem.as_mut_slice(),
        };

        let mut data = [0u8; 8];
        let result = sram.read_at(data.as_mut_slice(), 0);
        assert!(result.is_ok());
        let buf: [u32; 2] = transmute!(data);
        assert_eq!(&buf, &[0xdeadbeef, 0xfeedface]);

        let mut data = [0u8; 8];
        let result = sram.read_at(data.as_mut_slice(), 3);
        assert!(result.is_ok());
        let buf: [u32; 2] = transmute!(data);
        assert_eq!(&buf, &[0xedfacede, 0xadcafefe]);

        let mut data = [0u8; 5];
        let result = sram.read_at(data.as_mut_slice(), 0);
        assert!(result.is_ok());
        assert_eq!(&data, &[0xef, 0xbe, 0xad, 0xde, 0xce]);

        let mut data = [0u8; 3];
        let result = sram.read_at(data.as_mut_slice(), 4);
        assert!(result.is_ok());
        assert_eq!(&data, &[0xce, 0xfa, 0xed]);
    }

    #[test]
    fn test_respbuf_read_invalid_cases() {
        let mut mem = [0xdeadbeef, 0xfeedface, 0xabadcafe];
        let sram = MailboxRam {
            mem: mem.as_mut_slice(),
        };

        let mut data = [0u8; 50];
        let result = sram.read_at(data.as_mut_slice(), 0);
        assert_eq!(result, Err(DpeErrorCode::InvalidResponseBuf));

        let mut data = [0u8; 8];
        let result = sram.read_at(data.as_mut_slice(), 9999);
        assert_eq!(result, Err(DpeErrorCode::InvalidResponseBuf));

        let result = sram.read_at(data.as_mut_slice(), 10);
        assert_eq!(result, Err(DpeErrorCode::InvalidResponseBuf));
    }
}
