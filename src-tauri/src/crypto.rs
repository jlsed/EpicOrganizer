use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::cipher::consts::U12;
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use zeroize::Zeroizing;

/// File layout: MAGIC | salt | wrap-nonce | wrapped-DEK+tag | content-nonce | ciphertext+tag.
pub const MAGIC: &[u8; 5] = b"EORG1";
pub const EXTENSION: &str = "enc";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const DEK_LEN: usize = 32;
const TAG_LEN: usize = 16;
const WRAPPED_DEK_LEN: usize = DEK_LEN + TAG_LEN;
const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN + WRAPPED_DEK_LEN + NONCE_LEN;
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct EncryptionOutcome {
    pub output: PathBuf,
    pub plaintext_removed: bool,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct DecryptionOutcome {
    pub output: PathBuf,
    pub bytes: u64,
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| format!("no secure randomness available: {e}"))?;
    Ok(bytes)
}

/// Argon2id (v19) with OWASP-recommended parameters; the per-file salt makes
/// every KEK unique.
fn derive_kek(passphrase: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let mut kek = Zeroizing::new([0u8; 32]);
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut kek[..])
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(kek)
}

fn nonce_from(bytes: &[u8; NONCE_LEN]) -> Nonce<U12> {
    Nonce::<U12>::from(*bytes)
}

fn cipher_for(kek: &[u8; 32]) -> Result<Aes256Gcm, String> {
    Aes256Gcm::new_from_slice(&kek[..]).map_err(|_| "invalid key length".to_string())
}

fn seal(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let salt = random_bytes::<SALT_LEN>()?;
    let wrap_nonce = random_bytes::<NONCE_LEN>()?;
    let content_nonce = random_bytes::<NONCE_LEN>()?;
    let dek = Zeroizing::new(random_bytes::<DEK_LEN>()?);
    let kek = derive_kek(passphrase, &salt)?;
    let cipher = cipher_for(&kek)?;

    let wrapped = cipher
        .encrypt(&nonce_from(&wrap_nonce), &dek[..])
        .map_err(|_| "could not wrap the data key".to_string())?;
    let ciphertext = cipher
        .encrypt(&nonce_from(&content_nonce), plaintext)
        .map_err(|_| "could not encrypt the file contents".to_string())?;

    let mut sealed = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    sealed.extend_from_slice(MAGIC);
    sealed.extend_from_slice(&salt);
    sealed.extend_from_slice(&wrap_nonce);
    sealed.extend_from_slice(&wrapped);
    sealed.extend_from_slice(&content_nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

fn open(sealed: &[u8], passphrase: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    if sealed.len() < HEADER_LEN + TAG_LEN {
        return Err("not a valid EpicOrganizer encrypted file".into());
    }
    if &sealed[..MAGIC.len()] != MAGIC {
        return Err("not a valid EpicOrganizer encrypted file".into());
    }
    let salt = &sealed[5..5 + SALT_LEN];
    let wrap_nonce = &sealed[5 + SALT_LEN..21 + NONCE_LEN];
    let wrapped = &sealed[5 + SALT_LEN + NONCE_LEN..5 + SALT_LEN + NONCE_LEN + WRAPPED_DEK_LEN];
    let content_nonce = &sealed
        [5 + SALT_LEN + NONCE_LEN + WRAPPED_DEK_LEN..5 + SALT_LEN + NONCE_LEN + WRAPPED_DEK_LEN + NONCE_LEN];
    let ciphertext = &sealed[HEADER_LEN..];

    let kek = derive_kek(passphrase, salt)?;
    let cipher = cipher_for(&kek)?;
    let wrap_nonce: [u8; NONCE_LEN] = wrap_nonce
        .try_into()
        .map_err(|_| "corrupted header".to_string())?;
    let content_nonce: [u8; NONCE_LEN] = content_nonce
        .try_into()
        .map_err(|_| "corrupted header".to_string())?;

    let wrong = || "wrong passphrase or corrupted file".to_string();
    let dek = Zeroizing::new(
        cipher
            .decrypt(&nonce_from(&wrap_nonce), wrapped)
            .map_err(|_| wrong())?,
    );
    if dek.len() != DEK_LEN {
        return Err(wrong());
    }
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(&nonce_from(&content_nonce), ciphertext)
            .map_err(|_| wrong())?,
    );
    Ok(plaintext)
}

pub fn encrypted_path(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("'{}' has no file name", path.display()))?;
    let mut sealed_name = name.to_os_string();
    sealed_name.push(".enc");
    Ok(path.with_file_name(sealed_name))
}

pub fn restored_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    if name.len() <= 4 || !name.to_ascii_lowercase().ends_with(".enc") {
        return None;
    }
    let restored = &name[..name.len() - 4];
    if restored.is_empty() {
        return None;
    }
    Some(path.with_file_name(restored))
}

fn write_durably(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|e| format!("cannot write '{}': {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("cannot write '{}': {e}", path.display()))?;
    file.sync_all()
        .map_err(|e| format!("cannot flush '{}': {e}", path.display()))?;
    Ok(())
}

/// Encrypt `path` to `<name>.enc`, delete the plaintext after the sealed copy
/// is durable, and report whether that deletion succeeded.
pub fn encrypt_file(path: &Path, passphrase: &str) -> Result<EncryptionOutcome, String> {
    if passphrase.is_empty() {
        return Err("passphrase must not be empty".into());
    }
    let metadata = fs::metadata(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("'{}' is not a file", path.display()));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "'{}' is larger than the {} MiB limit for this build",
            path.display(),
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(EXTENSION))
    {
        return Err(format!("'{}' is already encrypted", path.display()));
    }

    let output = encrypted_path(path)?;
    if output.exists() {
        return Err(format!("'{}' already exists — remove or rename it first", output.display()));
    }

    let plaintext = Zeroizing::new(
        fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?,
    );
    let sealed = seal(&plaintext, passphrase)?;

    let temp = output.with_file_name(format!(
        "{}.part",
        output.file_name().and_then(|n| n.to_str()).unwrap_or("file.enc")
    ));
    let _ = fs::remove_file(&temp);
    if let Err(error) = write_durably(&temp, &sealed) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp, &output) {
        let _ = fs::remove_file(&temp);
        return Err(format!("cannot finalize '{}': {error}", output.display()));
    }

    let plaintext_removed = fs::remove_file(path).is_ok();
    Ok(EncryptionOutcome {
        output,
        plaintext_removed,
        bytes: metadata.len(),
    })
}

/// Decrypt a `.enc` file back to its original name. The encrypted copy is kept
/// (decryption is recovery, never destruction); an existing target is refused.
pub fn decrypt_file(path: &Path, passphrase: &str) -> Result<DecryptionOutcome, String> {
    if passphrase.is_empty() {
        return Err("passphrase must not be empty".into());
    }
    let output = restored_path(path)
        .ok_or_else(|| format!("'{}' is not an encrypted .enc file", path.display()))?;
    if output.exists() {
        return Err(format!("'{}' already exists — remove or rename it first", output.display()));
    }
    let sealed = fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    let plaintext = open(&sealed, passphrase)?;
    write_durably(&output, &plaintext)?;
    Ok(DecryptionOutcome {
        output,
        bytes: plaintext.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_bytes() -> Vec<u8> {
        (0u8..=255).cycle().take(4096).collect()
    }

    #[test]
    fn round_trip_restores_exact_bytes_and_removes_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("report.2024.txt");
        let content = sample_bytes();
        fs::write(&plain, &content).unwrap();

        let outcome = encrypt_file(&plain, "correct horse battery staple").unwrap();
        assert!(outcome.plaintext_removed);
        assert_eq!(outcome.bytes, content.len() as u64);
        assert!(!plain.exists());
        assert_eq!(outcome.output.file_name().unwrap(), "report.2024.txt.enc");

        let sealed = fs::read(&outcome.output).unwrap();
        assert_eq!(&sealed[..5], MAGIC);
        assert_eq!(sealed.len(), HEADER_LEN + content.len() + TAG_LEN);

        let restored = decrypt_file(&outcome.output, "correct horse battery staple").unwrap();
        assert_eq!(restored.output.file_name().unwrap(), "report.2024.txt");
        assert_eq!(fs::read(&restored.output).unwrap(), content);
        assert!(outcome.output.exists(), "decrypt must keep the encrypted copy");
    }

    #[test]
    fn wrong_passphrase_fails_cleanly_without_output() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.txt");
        fs::write(&plain, b"top secret").unwrap();
        let outcome = encrypt_file(&plain, "right").unwrap();

        let error = decrypt_file(&outcome.output, "wrong").unwrap_err();
        assert!(error.contains("wrong passphrase"), "unexpected error: {error}");
        assert!(!dir.path().join("a.txt").exists());
        assert!(outcome.output.exists());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.txt");
        fs::write(&plain, b"integrity matters").unwrap();
        let outcome = encrypt_file(&plain, "pass").unwrap();

        let mut sealed = fs::read(&outcome.output).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0xff;
        fs::write(&outcome.output, &sealed).unwrap();

        let error = decrypt_file(&outcome.output, "pass").unwrap_err();
        assert!(error.contains("wrong passphrase"), "unexpected error: {error}");
        assert!(!dir.path().join("a.txt").exists());
    }

    #[test]
    fn refuses_existing_targets_and_double_encryption() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.txt");
        fs::write(&plain, b"one").unwrap();
        fs::write(dir.path().join("a.txt.enc"), b"occupied").unwrap();
        let error = encrypt_file(&plain, "pass").unwrap_err();
        assert!(error.contains("already exists"), "unexpected error: {error}");

        let encrypted = dir.path().join("b.txt.enc");
        fs::write(&encrypted, b"sealed").unwrap();
        let error = encrypt_file(&encrypted, "pass").unwrap_err();
        assert!(error.contains("already encrypted"), "unexpected error: {error}");
    }

    #[test]
    fn rejects_empty_passphrase_and_non_encrypted_input() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.txt");
        fs::write(&plain, b"x").unwrap();
        assert!(encrypt_file(&plain, "").is_err());

        let error = decrypt_file(&plain, "pass").unwrap_err();
        assert!(error.contains("not an encrypted"), "unexpected error: {error}");

        let fake = dir.path().join("fake.enc");
        fs::write(&fake, b"not an EORG1 file at all").unwrap();
        let error = decrypt_file(&fake, "pass").unwrap_err();
        assert!(error.contains("not a valid"), "unexpected error: {error}");
    }
}
