use std::{error::Error, path::Path, num::NonZeroU32, fs::File, io::Write};

use ring::{digest::{Context, SHA256}, aead::OpeningKey};
use ring::{pbkdf2, digest, aead::{SealingKey, AES_256_GCM, UnboundKey, BoundKey, Nonce, NonceSequence, Aad, NONCE_LEN}, error::Unspecified};
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair};
use ripemd::{Ripemd160, Digest};
use base58check::{ToBase58Check, FromBase58Check, FromBase58CheckError};

/// Bitcoin uses a version prefix of 0x00 for wallets and 0x05 for P2SH addresses (and some other prefixes for other things).
/// None of the values in between are used as far as we know, so we took 0x03 for
/// our addresses so that they would start with a 2. Bitcoin addresses start with a 1
/// because of the 0x00 prefix.
const B58C_VERSION_PREFIX: u8 = 0x03;

static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;
static PBKDF2_ROUNDS: u32 = 100_000;
const CREDENTIAL_LEN: usize = digest::SHA256_OUTPUT_LEN;

/// We use the same nonce to generate the AES key to encrypt the private key file
/// because the key needs to be a deterministic function of the password
const AES_NONCE: [u8; NONCE_LEN] = [0x64; NONCE_LEN];

pub type Key = [u8; CREDENTIAL_LEN];

pub type Hash160 = [u8; 20];
pub type Hash256 = [u8; 32];

pub type Address = Hash160;

struct NonceGen {
}

impl NonceSequence for NonceGen {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        Ok(Nonce::assume_unique_for_key(AES_NONCE))
    }
}

pub fn load_keypair(password: &str, path: &str) -> Result<EcdsaKeyPair, Box<dyn Error>> {
    let mut keypair_ciphertext = std::fs::read(Path::new(path))?;
    let salt: [u8; 16] = salt_from_password(password);
    let rounds = NonZeroU32::new(PBKDF2_ROUNDS).unwrap();
    let mut key: Key = [0; CREDENTIAL_LEN];
    pbkdf2::derive(PBKDF2_ALG, rounds, &salt, password.as_bytes(), &mut key);

    let unbound_key = UnboundKey::new(&AES_256_GCM, &key).expect("Failed to create symmetric key");
    let mut opening_key = OpeningKey::new(unbound_key, NonceGen{});

    let keypair_decrypted = opening_key.open_in_place(Aad::empty(), &mut keypair_ciphertext).expect("Failed to decrypt keypair file");
    let alg = &ECDSA_P256_SHA256_ASN1_SIGNING;
    let keypair = EcdsaKeyPair::from_pkcs8(alg, &keypair_decrypted).expect("Failed to create ECDSA keypair");

    Ok(keypair)
}

pub fn create_keypair(password: &str, save_to: &str) -> Result<EcdsaKeyPair, Box<dyn Error>> {
    if Path::new(save_to).exists() {
        return Err(format!("Keypair already exists at {}", save_to))?;
    }

    let salt: [u8; 16] = salt_from_password(password);
    let rounds = NonZeroU32::new(PBKDF2_ROUNDS).unwrap();
    let mut key: Key = [0; CREDENTIAL_LEN];
    pbkdf2::derive(PBKDF2_ALG, rounds, &salt, password.as_bytes(), &mut key);
    
    let rng = ring::rand::SystemRandom::new();
    let alg = &ECDSA_P256_SHA256_ASN1_SIGNING;
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(alg, &rng).expect("Failed to generate ECDSA pkcs8");
    let keypair = EcdsaKeyPair::from_pkcs8(alg, pkcs8.as_ref()).expect("Failed to create ECDSA keypair");

    let unbound_key = UnboundKey::new(&AES_256_GCM, &key).expect("Failed to create symmetric key");
    let mut sealing_key = SealingKey::new(unbound_key, NonceGen{});

    let mut data = pkcs8.as_ref().to_vec();
    sealing_key.seal_in_place_append_tag(Aad::empty(), &mut data).unwrap();

    let mut keypair_file = File::create(save_to).expect("Failed to create keypair file");
    keypair_file.write_all(&data).expect("Failed to write to keypair file");

    Ok(keypair)
}

pub fn address_from_public_key(public_key: &Vec<u8>) -> Address {
    let mut context = Context::new(&SHA256);
    context.update(&public_key);
    let digest = context.finish();
    let sha256_hash = digest.as_ref();

    let mut hasher160 = Ripemd160::new();
    hasher160.update(sha256_hash);
    let result = hasher160.finalize().to_vec();

    let mut out = [0 as u8; 20];
    out.copy_from_slice(&result);

    out
}

pub fn address_to_b58c(address: &Vec<u8>) -> String {
    address.to_base58check(B58C_VERSION_PREFIX)
}

pub fn b58c_to_address(b58c: String) -> Result<Address, Box<dyn Error>> {
    let res = b58c.from_base58check();

    match res {
        Err(FromBase58CheckError::InvalidChecksum) => Err("Invalid checksum")?,
        Err(FromBase58CheckError::InvalidBase58(_)) => Err("Invalid base58")?,
        Ok((version, _)) if version != B58C_VERSION_PREFIX => Err("Invalid version")?,
        Ok((_, bytes)) => {
            let offset = 20 - bytes.len();
            let mut out = [0 as u8; 20];

            // Pad 20-byte integer with correct number of zeros
            out[offset..].copy_from_slice(&bytes);

            Ok(out)
        },
    }
}

fn salt_from_password(password: &str) -> [u8; 16] {
    let digest = ring::digest::digest(&digest::SHA256, password.as_bytes());
    let mut out = [0 as u8; 16];
    out.copy_from_slice(&digest.as_ref()[0..16]);

    out
}
