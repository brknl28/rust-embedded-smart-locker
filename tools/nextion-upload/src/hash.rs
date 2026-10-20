use std::ptr::{null, null_mut};
use windows_sys::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptHash, BCryptOpenAlgorithmProvider, BCRYPT_SHA256_ALGORITHM,
};

/// Use Windows CNG, not a project-specific cryptographic implementation.
pub fn sha256(bytes: &[u8]) -> Result<String, String> {
    let len = u32::try_from(bytes.len()).map_err(|_| "File exceeds CNG input limit")?;
    let mut handle = null_mut();
    let mut digest = [0u8; 32];
    // SAFETY: valid output handle, NUL-terminated SHA256 identifier, no optional provider.
    let status =
        unsafe { BCryptOpenAlgorithmProvider(&mut handle, BCRYPT_SHA256_ALGORITHM, null(), 0) };
    if status < 0 {
        return Err(format!("CNG provider failed: {status:#x}"));
    }
    // SAFETY: the provider is live; input/output pointers remain valid for their lengths.
    // BCryptHash documents that the input buffer is not modified. No HMAC secret is used.
    let status = unsafe {
        BCryptHash(
            handle,
            null(),
            0,
            bytes.as_ptr(),
            len,
            digest.as_mut_ptr(),
            32,
        )
    };
    // SAFETY: close the provider opened above exactly once, also on hashing failure.
    unsafe { BCryptCloseAlgorithmProvider(handle, 0) };
    if status < 0 {
        return Err(format!("CNG hash failed: {status:#x}"));
    }
    Ok(digest.iter().map(|b| format!("{b:02X}")).collect())
}
