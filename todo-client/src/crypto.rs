use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bip39::{Language, Mnemonic};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};

use crate::EncryptedField;

const ENCRYPTION_VERSION: u8 = 1;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

#[derive(Debug, Clone)]
pub struct CryptoKey {
    key: [u8; KEY_LEN],
    user_id: String,
}

impl CryptoKey {
    pub fn from_recovery_phrase(phrase: &str) -> Self {
        let normalized = normalize_recovery_phrase(phrase);
        let key = *blake3::hash(format!("encryption v1:{normalized}").as_bytes()).as_bytes();
        let user_id = blake3::hash(format!("user id v1:{normalized}").as_bytes())
            .to_hex()
            .to_string();

        Self { key, user_id }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn encrypt_field(&self, plaintext: &str) -> anyhow::Result<EncryptedField> {
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let mut nonce = [0; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);

        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|err| anyhow::anyhow!("encrypt field: {err}"))?;

        Ok(EncryptedField {
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            version: ENCRYPTION_VERSION,
        })
    }

    pub fn decrypt_field(&self, field: &EncryptedField) -> anyhow::Result<String> {
        if field.version != ENCRYPTION_VERSION {
            anyhow::bail!("unsupported encrypted field version {}", field.version);
        }

        let nonce = URL_SAFE_NO_PAD.decode(field.nonce.as_bytes())?;
        let ciphertext = URL_SAFE_NO_PAD.decode(field.ciphertext.as_bytes())?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());

        let plaintext = cipher
            .decrypt(XNonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|err| anyhow::anyhow!("decrypt field: {err}"))?;

        String::from_utf8(plaintext).map_err(Into::into)
    }
}

pub fn generate_recovery_phrase() -> String {
    Mnemonic::generate_in(Language::English, 12)
        .expect("word count is valid")
        .to_string()
}

pub fn normalize_phrase_for_storage(phrase: &str) -> Option<String> {
    let phrase = phrase.to_lowercase();
    let mnemonic = Mnemonic::parse_in(Language::English, phrase).ok()?;
    (mnemonic.word_count() == 12).then(|| mnemonic.to_string())
}

fn normalize_recovery_phrase(phrase: &str) -> String {
    phrase
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}
