//! Windows DPAPI encryption for sessionKey storage.

use windows::Win32::Security::Cryptography::*;
use windows::Win32::Foundation::*;

pub fn encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();

        CryptProtectData(
            &input, None, None, None, None, 0, &mut output,
        ).map_err(|_| "CryptProtectData failed")?;

        let encrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(encrypted)
    }
}

pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();

        CryptUnprotectData(
            &input, None, None, None, None, 0, &mut output,
        ).map_err(|_| "CryptUnprotectData failed")?;

        let decrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(decrypted)
    }
}
