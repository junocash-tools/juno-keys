#![deny(warnings)]

pub mod zip316;

use base64::Engine as _;
use orchard::keys::{FullViewingKey, SpendingKey};
use rand::RngCore as _;
use thiserror::Error;
use zeroize::Zeroize;
use zeroize::Zeroizing;

const TYPECODE_ORCHARD: u64 = 3;
const ORCHARD_FVK_LEN: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

impl Network {
    pub fn ua_hrp(&self) -> &'static str {
        match self {
            Network::Mainnet => "j",
            Network::Testnet => "jtest",
            Network::Regtest => "jregtest",
        }
    }

    pub fn coin_type(&self) -> u32 {
        match self {
            Network::Mainnet => 8133,
            Network::Testnet => 8134,
            Network::Regtest => 8135,
        }
    }
}

#[derive(Debug, Error)]
pub enum KeysError {
    #[error("seed_invalid")]
    SeedInvalid,
    #[error("ua_hrp_invalid")]
    UAHrpInvalid,
    #[error("coin_type_invalid")]
    CoinTypeInvalid,
    #[error("account_invalid")]
    AccountInvalid,
    #[error("internal")]
    Internal,
}

impl KeysError {
    pub fn code(&self) -> &'static str {
        match self {
            KeysError::SeedInvalid => "seed_invalid",
            KeysError::UAHrpInvalid => "ua_hrp_invalid",
            KeysError::CoinTypeInvalid => "coin_type_invalid",
            KeysError::AccountInvalid => "account_invalid",
            KeysError::Internal => "internal",
        }
    }
}

pub fn generate_seed_base64(bytes: usize) -> Result<Zeroizing<String>, KeysError> {
    if !(32..=252).contains(&bytes) {
        return Err(KeysError::SeedInvalid);
    }

    let mut seed = Zeroizing::new(vec![0u8; bytes]);
    rand::rngs::OsRng.fill_bytes(seed.as_mut_slice());

    let b64 = base64::engine::general_purpose::STANDARD.encode(seed.as_slice());
    Ok(Zeroizing::new(b64))
}

pub fn decode_seed_base64(seed_base64: &str) -> Result<Zeroizing<Vec<u8>>, KeysError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(seed_base64.trim())
        .map_err(|_| KeysError::SeedInvalid)?;
    if !(32..=252).contains(&bytes.len()) {
        return Err(KeysError::SeedInvalid);
    }
    Ok(Zeroizing::new(bytes))
}

pub fn ufvk_hrp_from_ua_hrp(ua_hrp: &str) -> Result<String, KeysError> {
    let hrp = ua_hrp.trim();
    if hrp.is_empty() {
        return Err(KeysError::UAHrpInvalid);
    }
    if hrp == "j" {
        return Ok("jview".to_string());
    }
    let Some(suffix) = hrp.strip_prefix('j') else {
        return Err(KeysError::UAHrpInvalid);
    };
    if suffix.is_empty() {
        return Ok("jview".to_string());
    }
    Ok(format!("jview{suffix}"))
}

pub fn ufvk_from_seed_base64(
    seed_base64: &str,
    ua_hrp: &str,
    coin_type: u32,
    account: u32,
) -> Result<String, KeysError> {
    if coin_type >= 0x8000_0000 {
        return Err(KeysError::CoinTypeInvalid);
    }
    if account >= 0x8000_0000 {
        return Err(KeysError::AccountInvalid);
    }

    let ufvk_hrp = ufvk_hrp_from_ua_hrp(ua_hrp)?;

    let mut seed = decode_seed_base64(seed_base64)?;
    let account = zip32::AccountId::try_from(account).map_err(|_| KeysError::AccountInvalid)?;
    let sk = SpendingKey::from_zip32_seed(seed.as_slice(), coin_type, account)
        .map_err(|_| KeysError::SeedInvalid)?;
    seed.zeroize();

    let fvk = FullViewingKey::from(&sk);
    let fvk_bytes = fvk.to_bytes();
    if fvk_bytes.len() != ORCHARD_FVK_LEN {
        return Err(KeysError::Internal);
    }

    zip316::encode_unified_container(&ufvk_hrp, TYPECODE_ORCHARD, &fvk_bytes)
        .map_err(|_| KeysError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_roundtrip_default_len() {
        let seed_b64 = generate_seed_base64(64).expect("seed");
        let seed = decode_seed_base64(&seed_b64).expect("decode");
        assert_eq!(seed.len(), 64);
    }

    #[test]
    fn derives_ufvk_prefixes() {
        let seed = [7u8; 64];
        let seed_b64 = base64::engine::general_purpose::STANDARD.encode(seed);

        let ufvk_main = ufvk_from_seed_base64(
            &seed_b64,
            Network::Mainnet.ua_hrp(),
            Network::Mainnet.coin_type(),
            0,
        )
        .expect("ufvk main");
        assert!(ufvk_main.starts_with("jview1"));

        let ufvk_regtest = ufvk_from_seed_base64(
            &seed_b64,
            Network::Regtest.ua_hrp(),
            Network::Regtest.coin_type(),
            0,
        )
        .expect("ufvk regtest");
        assert!(ufvk_regtest.starts_with("jviewregtest1"));
    }

    #[test]
    fn ufvk_from_seed_rejects_invalid_coin_type() {
        let seed = [7u8; 64];
        let seed_b64 = base64::engine::general_purpose::STANDARD.encode(seed);
        let err = ufvk_from_seed_base64(&seed_b64, "j", 0x8000_0000, 0).expect_err("err");
        assert!(matches!(err, KeysError::CoinTypeInvalid));
    }

    #[test]
    fn ufvk_from_seed_rejects_invalid_ua_hrp() {
        let seed = [7u8; 64];
        let seed_b64 = base64::engine::general_purpose::STANDARD.encode(seed);
        let err = ufvk_from_seed_base64(&seed_b64, "x", 8133, 0).expect_err("err");
        assert!(matches!(err, KeysError::UAHrpInvalid));
    }
}
