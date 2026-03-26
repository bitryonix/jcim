#![allow(clippy::missing_docs_in_private_items)]

use std::io::Error;
use std::path::PathBuf;

use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use cbc::{Decryptor, Encryptor};
use hmac::{Hmac, Mac};
use jcim_sdk::{Aid, CardConnection, CommandApdu, ResponseApdu, iso7816};
use rand::RngCore;
use rand::rngs::OsRng;
use secp256k1::ecdh::shared_secret_point;
use secp256k1::ecdsa::Signature;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sha1::Sha1;
use sha2::{Digest, Sha256};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type HmacSha1 = Hmac<Sha1>;
type SignedXPayload<'a> = ([u8; 32], &'a [u8], &'a [u8]);

const SATOCHIP_CLA: u8 = 0xB0;
const INS_SETUP: u8 = 0x2A;
const INS_GET_STATUS: u8 = 0x3C;
const INS_VERIFY_PIN: u8 = 0x42;
const INS_BIP32_IMPORT_SEED: u8 = 0x6C;
const INS_BIP32_GET_EXTENDED_KEY: u8 = 0x6D;
const INS_SIGN_TRANSACTION_HASH: u8 = 0x7A;
const INS_INIT_SECURE_CHANNEL: u8 = 0x81;
const INS_PROCESS_SECURE_CHANNEL: u8 = 0x82;
const SC_KEY_LABEL: &[u8; 6] = b"sc_key";
const SC_MAC_LABEL: &[u8; 6] = b"sc_mac";
const DEFAULT_PIN: &[u8; 8] = b"Muscle00";
const PRIMARY_PIN: &[u8; 6] = b"123456";
const PRIMARY_UBLK: &[u8; 8] = b"12345678";
const SECONDARY_PIN: &[u8; 6] = b"654321";
const SECONDARY_UBLK: &[u8; 8] = b"87654321";
const SECMEM_SIZE: u16 = 1024;
const DEMO_PATH: [u32; 5] = [0x8000_0000 | 84, 0x8000_0000, 0x8000_0000, 0, 0];
const DEMO_SEED_HEX: &str = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F";
const DEMO_TRANSACTION_HEX: &str = concat!(
    "0100000001",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "FFFFFFFF",
    "00",
    "FFFFFFFF",
    "01",
    "00E1F50500000000",
    "19",
    "76A91400112233445566778899AABBCCDDEEFF0011223388AC",
    "00000000"
);

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SatochipStatus {
    pub(crate) protocol_version: (u8, u8),
    pub(crate) applet_version: (u8, u8),
    pub(crate) pin0_tries_remaining: u8,
    pub(crate) puk0_tries_remaining: u8,
    pub(crate) pin1_tries_remaining: u8,
    pub(crate) puk1_tries_remaining: u8,
    pub(crate) needs_2fa: bool,
    pub(crate) seeded: bool,
    pub(crate) setup_done: bool,
    pub(crate) needs_secure_channel: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SatochipFlowSummary {
    pub(crate) initial_status: SatochipStatus,
    pub(crate) post_setup_status: SatochipStatus,
    pub(crate) post_seed_status: SatochipStatus,
    pub(crate) authentikey_hex: String,
    pub(crate) derived_pubkey_hex: String,
    pub(crate) chain_code_hex: String,
    pub(crate) transaction_hex: String,
    pub(crate) transaction_hash_hex: String,
    pub(crate) signature_hex: String,
}

struct RecoveredKey {
    encoded_hex: String,
}

struct DerivedKey {
    public_key: PublicKey,
    encoded_hex: String,
    chain_code_hex: String,
}

struct SecureChannelState {
    session_key: [u8; 16],
    mac_key: [u8; 20],
    next_host_counter: u32,
    last_card_counter: u32,
}

pub(crate) fn satochip_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir")
}

pub(crate) async fn run_wallet_demo(
    connection: &CardConnection,
) -> Result<SatochipFlowSummary, BoxError> {
    ensure_success("select Satochip", &select_satochip(connection).await?)?;

    let initial_status = get_status(connection).await?;
    if initial_status.setup_done || initial_status.seeded {
        return Err(example_error(
            "the Satochip wallet demo expects a fresh applet; start a new simulation or install onto a fresh card",
        ));
    }
    if !initial_status.needs_secure_channel {
        return Err(example_error(
            "the Satochip applet reported that secure channel is disabled; this demo expects the maintained secure-channel path",
        ));
    }

    let mut secure_channel = open_secure_channel(connection).await?;
    ensure_success(
        "setup wallet",
        &setup_wallet(connection, &mut secure_channel).await?,
    )?;

    let post_setup_status = get_status(connection).await?;
    if !post_setup_status.setup_done {
        return Err(example_error(
            "wallet setup reported success but the applet status still says setup is incomplete",
        ));
    }

    ensure_success(
        "verify primary PIN",
        &verify_primary_pin(connection, &mut secure_channel).await?,
    )?;

    let authentikey = import_demo_seed(connection, &mut secure_channel).await?;
    let post_seed_status = get_status(connection).await?;
    if !post_seed_status.seeded {
        return Err(example_error(
            "seed import reported success but the applet status still says the wallet is unseeded",
        ));
    }

    let derived_key = derive_demo_key(connection, &mut secure_channel).await?;
    let transaction_hex = DEMO_TRANSACTION_HEX.to_string();
    let transaction_hash = demo_transaction_hash()?;
    let signature =
        sign_transaction_hash(connection, &mut secure_channel, &transaction_hash).await?;
    verify_signature(
        "transaction signature",
        &derived_key.public_key,
        &transaction_hash,
        &signature,
    )?;

    Ok(SatochipFlowSummary {
        initial_status,
        post_setup_status,
        post_seed_status,
        authentikey_hex: authentikey.encoded_hex,
        derived_pubkey_hex: derived_key.encoded_hex,
        chain_code_hex: derived_key.chain_code_hex,
        transaction_hex,
        transaction_hash_hex: hex::encode_upper(transaction_hash),
        signature_hex: hex::encode_upper(signature),
    })
}

async fn select_satochip(connection: &CardConnection) -> Result<ResponseApdu, BoxError> {
    let name = Aid::from_slice(b"SatoChip\0")?;
    Ok(connection.transmit(&iso7816::select_by_name(&name)).await?)
}

async fn get_status(connection: &CardConnection) -> Result<SatochipStatus, BoxError> {
    let response = connection
        .transmit(&CommandApdu::new(
            SATOCHIP_CLA,
            INS_GET_STATUS,
            0x00,
            0x00,
            Vec::new(),
            Some(256),
        ))
        .await?;
    ensure_success("get status", &response)?;

    if response.data.len() < 12 {
        return Err(example_error(format!(
            "expected 12 status bytes from Satochip, got {}",
            response.data.len()
        )));
    }
    let data = &response.data;
    Ok(SatochipStatus {
        protocol_version: (data[0], data[1]),
        applet_version: (data[2], data[3]),
        pin0_tries_remaining: data[4],
        puk0_tries_remaining: data[5],
        pin1_tries_remaining: data[6],
        puk1_tries_remaining: data[7],
        needs_2fa: data[8] != 0,
        seeded: data[9] != 0,
        setup_done: data[10] != 0,
        needs_secure_channel: data[11] != 0,
    })
}

async fn open_secure_channel(connection: &CardConnection) -> Result<SecureChannelState, BoxError> {
    let secp = Secp256k1::new();
    let client_secret = random_secret_key()?;
    let client_public = PublicKey::from_secret_key(&secp, &client_secret);
    let response = connection
        .transmit(&CommandApdu::new(
            SATOCHIP_CLA,
            INS_INIT_SECURE_CHANNEL,
            0x00,
            0x00,
            client_public.serialize_uncompressed().to_vec(),
            None,
        ))
        .await?;
    ensure_success("open secure channel", &response)?;

    let (x_bytes, self_signature, _tail) = parse_x_and_signature(&response.data)?;
    let message = response
        .data
        .get(..34)
        .ok_or_else(|| example_error("secure-channel response is missing its signed header"))?;
    let card_public = recover_key_from_x(message, &x_bytes, self_signature)?;
    let shared_point = shared_secret_point(&card_public, &client_secret);
    let shared_x = &shared_point[..32];

    let session_key_material = hmac_sha1(shared_x, SC_KEY_LABEL)?;
    let mac_key_material = hmac_sha1(shared_x, SC_MAC_LABEL)?;

    let mut session_key = [0u8; 16];
    session_key.copy_from_slice(
        session_key_material
            .get(..16)
            .ok_or_else(|| example_error("secure-channel key derivation returned too few bytes"))?,
    );
    let mut mac_key = [0u8; 20];
    mac_key.copy_from_slice(&mac_key_material);

    Ok(SecureChannelState {
        session_key,
        mac_key,
        next_host_counter: 1,
        last_card_counter: 0,
    })
}

async fn setup_wallet(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
) -> Result<ResponseApdu, BoxError> {
    let mut payload = Vec::new();
    payload.push(DEFAULT_PIN.len() as u8);
    payload.extend_from_slice(DEFAULT_PIN);
    payload.extend_from_slice(&[5, 3, PRIMARY_PIN.len() as u8]);
    payload.extend_from_slice(PRIMARY_PIN);
    payload.push(PRIMARY_UBLK.len() as u8);
    payload.extend_from_slice(PRIMARY_UBLK);
    payload.extend_from_slice(&[5, 3, SECONDARY_PIN.len() as u8]);
    payload.extend_from_slice(SECONDARY_PIN);
    payload.push(SECONDARY_UBLK.len() as u8);
    payload.extend_from_slice(SECONDARY_UBLK);
    payload.extend_from_slice(&SECMEM_SIZE.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&[0x00, 0x00, 0x00]);
    payload.extend_from_slice(&0u16.to_be_bytes());

    transmit_secure(
        connection,
        secure_channel,
        &CommandApdu::new(SATOCHIP_CLA, INS_SETUP, 0x00, 0x00, payload, None),
    )
    .await
}

async fn verify_primary_pin(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
) -> Result<ResponseApdu, BoxError> {
    transmit_secure(
        connection,
        secure_channel,
        &CommandApdu::new(
            SATOCHIP_CLA,
            INS_VERIFY_PIN,
            0x00,
            0x00,
            PRIMARY_PIN.to_vec(),
            None,
        ),
    )
    .await
}

async fn import_demo_seed(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
) -> Result<RecoveredKey, BoxError> {
    let seed = hex::decode(DEMO_SEED_HEX)?;
    let response = transmit_secure(
        connection,
        secure_channel,
        &CommandApdu::new(
            SATOCHIP_CLA,
            INS_BIP32_IMPORT_SEED,
            u8::try_from(seed.len())
                .map_err(|_| example_error("demo seed does not fit in one APDU P1 length byte"))?,
            0x00,
            seed,
            None,
        ),
    )
    .await?;
    ensure_success("import demo seed", &response)?;
    parse_recovered_key(&response.data)
}

async fn derive_demo_key(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
) -> Result<DerivedKey, BoxError> {
    let response = transmit_secure(
        connection,
        secure_channel,
        &CommandApdu::new(
            SATOCHIP_CLA,
            INS_BIP32_GET_EXTENDED_KEY,
            DEMO_PATH.len() as u8,
            0x00,
            encode_path(&DEMO_PATH),
            None,
        ),
    )
    .await?;
    ensure_success("derive demo key", &response)?;
    parse_derived_key(&response.data)
}

async fn sign_transaction_hash(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
    hash: &[u8; 32],
) -> Result<Vec<u8>, BoxError> {
    let response = transmit_secure(
        connection,
        secure_channel,
        &CommandApdu::new(
            SATOCHIP_CLA,
            INS_SIGN_TRANSACTION_HASH,
            0xFF,
            0x00,
            hash.to_vec(),
            None,
        ),
    )
    .await?;
    ensure_success("sign transaction hash", &response)?;
    Ok(response.data)
}

async fn transmit_secure(
    connection: &CardConnection,
    secure_channel: &mut SecureChannelState,
    inner: &CommandApdu,
) -> Result<ResponseApdu, BoxError> {
    let wrapped = secure_channel.wrap_command(inner)?;
    let response = connection.transmit(&wrapped).await?;
    if !response.is_success() {
        return Ok(response);
    }
    secure_channel.unwrap_response(&response)
}

impl SecureChannelState {
    fn wrap_command(&mut self, inner: &CommandApdu) -> Result<CommandApdu, BoxError> {
        let mut iv = [0u8; 16];
        OsRng.fill_bytes(&mut iv[..12]);
        iv[12..].copy_from_slice(&self.next_host_counter.to_be_bytes());
        self.next_host_counter = self
            .next_host_counter
            .checked_add(2)
            .ok_or_else(|| example_error("secure-channel command counter overflowed"))?;

        let ciphertext = encrypt_aes_cbc_pkcs7(&self.session_key, &iv, &inner.to_bytes())?;
        let size = u16::try_from(ciphertext.len()).map_err(|_| {
            example_error("wrapped secure-channel command exceeded Satochip short-length limits")
        })?;

        let mut data = Vec::with_capacity(16 + 2 + ciphertext.len() + 2 + 20);
        data.extend_from_slice(&iv);
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(&ciphertext);

        let mac = hmac_sha1(&self.mac_key, &data)?;
        data.extend_from_slice(&(mac.len() as u16).to_be_bytes());
        data.extend_from_slice(&mac);

        Ok(CommandApdu::new(
            SATOCHIP_CLA,
            INS_PROCESS_SECURE_CHANNEL,
            0x00,
            0x00,
            data,
            None,
        ))
    }

    fn unwrap_response(&mut self, response: &ResponseApdu) -> Result<ResponseApdu, BoxError> {
        if response.data.is_empty() {
            return Ok(ResponseApdu::success(Vec::new()));
        }

        if response.data.len() < 18 {
            return Err(example_error(format!(
                "secure-channel response is too short: {} byte(s)",
                response.data.len()
            )));
        }

        let iv = response
            .data
            .get(..16)
            .ok_or_else(|| example_error("secure-channel response is missing its IV"))?;
        let counter = u32::from_be_bytes(
            iv[12..16]
                .try_into()
                .map_err(|_| example_error("secure-channel IV is missing its counter bytes"))?,
        );
        if counter & 1 != 0 {
            return Err(example_error(
                "secure-channel response counter must be even but the card returned an odd IV counter",
            ));
        }
        if counter <= self.last_card_counter {
            return Err(example_error(
                "secure-channel response counter did not advance monotonically",
            ));
        }
        self.last_card_counter = counter;

        let encrypted_len =
            u16::from_be_bytes(response.data[16..18].try_into().map_err(|_| {
                example_error("secure-channel response is missing its length field")
            })?) as usize;
        let encrypted = response.data.get(18..18 + encrypted_len).ok_or_else(|| {
            example_error("secure-channel response ciphertext length is truncated")
        })?;
        let plaintext = decrypt_aes_cbc_pkcs7(&self.session_key, iv, encrypted)?;

        Ok(ResponseApdu::success(plaintext))
    }
}

fn parse_recovered_key(data: &[u8]) -> Result<RecoveredKey, BoxError> {
    let (x_bytes, self_signature, _tail) = parse_x_and_signature(data)?;
    let message = data
        .get(..34)
        .ok_or_else(|| example_error("recovered-key response is missing its signed header"))?;
    let public_key = recover_key_from_x(message, &x_bytes, self_signature)?;
    Ok(RecoveredKey {
        encoded_hex: hex::encode_upper(public_key.serialize_uncompressed()),
    })
}

fn parse_derived_key(data: &[u8]) -> Result<DerivedKey, BoxError> {
    if data.len() < 68 {
        return Err(example_error(format!(
            "derived-key response is too short: {} byte(s)",
            data.len()
        )));
    }

    let chain_code_hex = hex::encode_upper(&data[..32]);
    let x_len = u16::from_be_bytes(
        data[32..34]
            .try_into()
            .map_err(|_| example_error("derived-key response is missing its x length"))?,
    ) as usize;
    if x_len != 32 {
        return Err(example_error(format!(
            "derived-key response used unsupported x-coordinate length {x_len}",
        )));
    }

    let x_bytes: [u8; 32] = data[34..66]
        .try_into()
        .map_err(|_| example_error("derived-key response truncated its x coordinate"))?;
    let signature_len = u16::from_be_bytes(
        data[66..68]
            .try_into()
            .map_err(|_| example_error("derived-key response is missing its signature length"))?,
    ) as usize;
    let signature = data
        .get(68..68 + signature_len)
        .ok_or_else(|| example_error("derived-key response truncated its self-signature"))?;
    let public_key = recover_key_from_x(&data[..66], &x_bytes, signature)?;

    Ok(DerivedKey {
        encoded_hex: hex::encode_upper(public_key.serialize_uncompressed()),
        public_key,
        chain_code_hex,
    })
}

fn parse_x_and_signature(data: &[u8]) -> Result<SignedXPayload<'_>, BoxError> {
    if data.len() < 36 {
        return Err(example_error(format!(
            "secure-channel payload is too short: {} byte(s)",
            data.len()
        )));
    }

    let x_len = u16::from_be_bytes(
        data[0..2]
            .try_into()
            .map_err(|_| example_error("payload is missing its x length field"))?,
    ) as usize;
    if x_len != 32 {
        return Err(example_error(format!(
            "payload used unsupported x-coordinate length {x_len}",
        )));
    }

    let x_bytes = data
        .get(2..34)
        .ok_or_else(|| example_error("payload truncated its x coordinate"))?;
    let signature_len = u16::from_be_bytes(
        data[34..36]
            .try_into()
            .map_err(|_| example_error("payload is missing its signature length field"))?,
    ) as usize;
    let signature = data
        .get(36..36 + signature_len)
        .ok_or_else(|| example_error("payload truncated its self-signature"))?;
    let tail = data
        .get(36 + signature_len..)
        .ok_or_else(|| example_error("payload tail indexing failed unexpectedly"))?;
    Ok((
        x_bytes
            .try_into()
            .map_err(|_| example_error("payload x coordinate did not contain 32 bytes"))?,
        signature,
        tail,
    ))
}

fn recover_key_from_x(
    message: &[u8],
    x_bytes: &[u8; 32],
    signature_bytes: &[u8],
) -> Result<PublicKey, BoxError> {
    let secp = Secp256k1::new();
    let signature = Signature::from_der(signature_bytes)?;
    let digest: [u8; 32] = Sha256::digest(message).into();
    let message = Message::from_digest(digest);

    for prefix in [0x02_u8, 0x03_u8] {
        let mut encoded = [0u8; 33];
        encoded[0] = prefix;
        encoded[1..].copy_from_slice(x_bytes);
        if let Ok(candidate) = PublicKey::from_slice(&encoded)
            && secp.verify_ecdsa(message, &signature, &candidate).is_ok()
        {
            return Ok(candidate);
        }
    }

    Err(example_error(
        "could not recover a valid secp256k1 public key from the returned x coordinate and self-signature",
    ))
}

fn verify_signature(
    label: &str,
    public_key: &PublicKey,
    prehash: &[u8; 32],
    signature_bytes: &[u8],
) -> Result<(), BoxError> {
    let secp = Secp256k1::new();
    let signature = Signature::from_der(signature_bytes)?;
    secp.verify_ecdsa(Message::from_digest(*prehash), &signature, public_key)
        .map_err(|error| example_error(format!("{label} did not verify: {error}")))?;
    Ok(())
}

fn demo_transaction_hash() -> Result<[u8; 32], BoxError> {
    let transaction = hex::decode(DEMO_TRANSACTION_HEX)?;
    let first = Sha256::digest(&transaction);
    let second = Sha256::digest(first);
    Ok(second.into())
}

fn encode_path(path: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(path.len() * 4);
    for index in path {
        bytes.extend_from_slice(&index.to_be_bytes());
    }
    bytes
}

fn ensure_success(label: &str, response: &ResponseApdu) -> Result<(), BoxError> {
    if response.is_success() {
        return Ok(());
    }

    Err(example_error(format!(
        "{label} failed with status {:04X}",
        response.sw
    )))
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> Result<[u8; 20], BoxError> {
    let mut mac = HmacSha1::new_from_slice(key)
        .map_err(|error| example_error(format!("invalid HMAC-SHA1 key: {error}")))?;
    mac.update(message);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn encrypt_aes_cbc_pkcs7(key: &[u8; 16], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, BoxError> {
    let mut buffer = vec![0u8; plaintext.len() + 16];
    buffer[..plaintext.len()].copy_from_slice(plaintext);
    let encrypted = Encryptor::<Aes128>::new_from_slices(key, iv)
        .map_err(|error| example_error(format!("invalid AES-CBC encryption parameters: {error}")))?
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|error| example_error(format!("AES-CBC encryption failed: {error}")))?;
    Ok(encrypted.to_vec())
}

fn decrypt_aes_cbc_pkcs7(
    key: &[u8; 16],
    iv: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, BoxError> {
    let mut buffer = ciphertext.to_vec();
    let decrypted = Decryptor::<Aes128>::new_from_slices(key, iv)
        .map_err(|error| example_error(format!("invalid AES-CBC decryption parameters: {error}")))?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|error| example_error(format!("AES-CBC decryption failed: {error}")))?;
    Ok(decrypted.to_vec())
}

fn example_error(message: impl Into<String>) -> BoxError {
    Box::new(Error::other(message.into()))
}

fn random_secret_key() -> Result<SecretKey, BoxError> {
    loop {
        let mut candidate = [0u8; 32];
        OsRng.fill_bytes(&mut candidate);
        if let Ok(secret_key) = SecretKey::from_byte_array(candidate) {
            return Ok(secret_key);
        }
    }
}
