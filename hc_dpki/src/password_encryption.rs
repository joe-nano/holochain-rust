use holochain_core_types::error::HcResult;
use holochain_sodium::{aead, kx, pwhash, secbuf::SecBuf};
use crate::secbuf_utils::*;

pub type OpsLimit = u64;
pub type MemLimit = usize;
pub type PwHashAlgo = i8;

pub struct PwHashConfig(pub OpsLimit, pub MemLimit, pub PwHashAlgo);


/// Struct holding the result of a passphrase encryption
#[derive(RustcDecodable, RustcEncodable)]
pub(crate) struct EncryptedData {
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub cipher: Vec<u8>,
}


/// Simplify the API for generating a password hash with our set parameters
/// @param {SecBuf} pass - the password buffer to hash
/// @param {SecBuf} salt - if specified, hash with this salt (otherwise random)
/// @param {SecBuf} -  Empty hash buf
/// TODO make salt optional
pub(crate) fn pw_hash(
    password: &mut SecBuf,
    salt: &mut SecBuf,
    hash_result: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> HcResult<()> {
    debug_assert!(is_secbuf_empty(hash_result));
    let config = config.unwrap_or(PwHashConfig(
        pwhash::OPSLIMIT_SENSITIVE,
        pwhash::MEMLIMIT_SENSITIVE,
        pwhash::ALG_ARGON2ID13,
    ));
    pwhash::hash(password, config.0, config.1, config.2, salt, hash_result);
    debug_assert!(!is_secbuf_empty(hash_result));
    Ok(())
}

/// Helper for encrypting a buffer with a pwhash-ed passphrase
/// @param {Buffer} data
/// @param {string} passphrase
/// @return {BlobData} - the encrypted data
pub(crate) fn pw_enc(
    data: &mut SecBuf,
    passphrase: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> HcResult<EncryptedData> {
    let mut secret = SecBuf::with_secure(kx::SESSIONKEYBYTES);
    let mut salt = SecBuf::with_insecure(pwhash::SALTBYTES);
    salt.randomize();
    let mut nonce = SecBuf::with_insecure(aead::NONCEBYTES);
    nonce.randomize();
    let mut cipher = SecBuf::with_insecure(data.len() + aead::ABYTES);
    pw_hash(passphrase, &mut salt, &mut secret, config)?;
    aead::enc(data, &mut secret, None, &mut nonce, &mut cipher)?;

    let salt = salt.read_lock().to_vec();
    let nonce = nonce.read_lock().to_vec();
    let cipher = cipher.read_lock().to_vec();
    // Done
    Ok(EncryptedData { salt, nonce, cipher })
}

/// Helper for decrypting a buffer with a pwhash-ed passphrase
/// @param {Buffer} data
/// @param {string} passphrase
/// @param {SecBuf} - the decrypted data
pub(crate) fn pw_dec(
    encrypted_data: &EncryptedData,
    passphrase: &mut SecBuf,
    decrypted_data: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> HcResult<()> {
    let mut secret = SecBuf::with_secure(kx::SESSIONKEYBYTES);
    let mut salt = SecBuf::with_insecure(pwhash::SALTBYTES);
    vec_to_secbuf(&encrypted_data.salt, &mut salt);
    let mut nonce = SecBuf::with_insecure(encrypted_data.nonce.len());
    vec_to_secbuf(&encrypted_data.nonce, &mut nonce);
    let mut cipher = SecBuf::with_insecure(encrypted_data.cipher.len());
    vec_to_secbuf(&encrypted_data.cipher, &mut cipher);
    pw_hash(passphrase, &mut salt, &mut secret, config)?;
    aead::dec(decrypted_data, &mut secret, None, &mut nonce, &mut cipher)?;
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: Option<PwHashConfig> = Some(PwHashConfig(
        pwhash::OPSLIMIT_INTERACTIVE,
        pwhash::MEMLIMIT_INTERACTIVE,
        pwhash::ALG_ARGON2ID13,
    ));

    fn test_password() -> SecBuf {
        let mut password = SecBuf::with_insecure(pwhash::HASHBYTES);
        {
            let mut password = password.write_lock();
            password[0] = 42;
            password[1] = 222;
        }
        password
    }


    #[test]
    fn it_should_encrypt_data() {
        let mut password = test_password();
        let mut data = SecBuf::with_insecure(32);
        {
            let mut data = data.write_lock();
            data[0] = 88;
            data[1] = 101;
        }
        let mut encrypted_data =
            pw_enc(&mut data, &mut password, TEST_CONFIG).unwrap();

        let mut decrypted_data = SecBuf::with_insecure(32);
        pw_dec(&mut encrypted_data, &mut password, &mut decrypted_data, TEST_CONFIG).unwrap();

        let data = data.read_lock();
        let decrypted_data = decrypted_data.read_lock();
        assert_eq!(format!("{:?}", *decrypted_data), format!("{:?}", *data));
    }

    #[test]
    fn it_should_generate_pw_hash_with_salt() {
        let mut password = test_password();
        let mut hashed_password = SecBuf::with_insecure(pwhash::HASHBYTES);
        let mut salt = SecBuf::with_insecure(pwhash::SALTBYTES);
        println!("salt = {:?}", salt);
        pw_hash(&mut password, &mut salt, &mut hashed_password, TEST_CONFIG).unwrap();
        println!("salt = {:?}", salt);
        let pw2_hash = hashed_password.read_lock();
        assert_eq!(
            "[134, 156, 170, 171, 184, 19, 40, 158, 64, 227, 105, 252, 59, 175, 119, 226, 77, 238, 49, 61, 27, 174, 47, 246, 179, 168, 88, 200, 65, 11, 14, 159]",
            format!("{:?}", *pw2_hash),
        );

        // TODO
        // hash again, should have different result
    }

}