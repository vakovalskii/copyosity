//! Pairing (SPAKE2 over a 6-digit code) and the encrypted frame channel.
//!
//! Pairing: both sides run symmetric SPAKE2 with the code as the password, so the code is
//! never sent and a wrong guess gives a different key. Each side then proves the key with an
//! encrypted confirmation; on success both derive the same 32-byte pair secret with HKDF over
//! the SPAKE2 key and both device ids. Only that secret is stored.
//!
//! Sessions: every connection mixes fresh nonces from both sides into per-direction keys
//! (HKDF over the pair secret), frames are ChaCha20-Poly1305 with a counter nonce. A peer that
//! does not hold the secret fails on the first frame.

use std::io::{Read, Write};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};

pub const MAX_FRAME: usize = 64 * 1024 * 1024;

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).expect("OS random source");
    hex::encode(buf)
}

pub fn random_code() -> String {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).expect("OS random source");
    format!("{:06}", u32::from_le_bytes(buf) % 1_000_000)
}

fn hkdf(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, &mut out)
        .expect("32 bytes is a valid HKDF length");
    out
}

/// Start SPAKE2; returns the state and the message to send to the other side.
pub fn pair_start(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(code.as_bytes()),
        &Identity::new(b"copyosity-lan-pair-v1"),
    )
}

pub fn pair_finish(state: Spake2<Ed25519Group>, inbound: &[u8]) -> Result<Vec<u8>, String> {
    state.finish(inbound).map_err(|e| format!("spake2: {e:?}"))
}

/// Pair secret both sides store: independent of who initiated.
pub fn pair_secret(spake_key: &[u8], device_a: &str, device_b: &str) -> [u8; 32] {
    let (lo, hi) = if device_a <= device_b {
        (device_a, device_b)
    } else {
        (device_b, device_a)
    };
    hkdf(
        spake_key,
        b"copyosity-pair-secret",
        format!("{lo}|{hi}").as_bytes(),
    )
}

pub fn confirm_tag(spake_key: &[u8], device_id: &str) -> Vec<u8> {
    hkdf(spake_key, b"copyosity-pair-confirm", device_id.as_bytes()).to_vec()
}

/// Per-direction keys for one session; `initiator` is the side that connected.
fn session_keys(
    secret: &[u8],
    nonce_client: &str,
    nonce_server: &str,
    initiator: bool,
) -> ([u8; 32], [u8; 32]) {
    let salt = format!("{nonce_client}|{nonce_server}");
    let c2s = hkdf(secret, salt.as_bytes(), b"c2s");
    let s2c = hkdf(secret, salt.as_bytes(), b"s2c");
    if initiator {
        (c2s, s2c)
    } else {
        (s2c, c2s)
    }
}

fn nonce(ctr: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&ctr.to_be_bytes());
    n
}

pub struct Sender<W: Write> {
    w: W,
    aead: ChaCha20Poly1305,
    ctr: u64,
}

pub struct Receiver<R: Read> {
    r: R,
    aead: ChaCha20Poly1305,
    ctr: u64,
}

/// Encrypted channel over a stream split into a writer and a reader (e.g. a TcpStream and
/// its `try_clone`), so one thread can send while another receives.
pub fn channel<W: Write, R: Read>(
    w: W,
    r: R,
    secret: &[u8],
    nonce_client: &str,
    nonce_server: &str,
    initiator: bool,
) -> (Sender<W>, Receiver<R>) {
    let (send, recv) = session_keys(secret, nonce_client, nonce_server, initiator);
    (
        Sender {
            w,
            aead: ChaCha20Poly1305::new(Key::from_slice(&send)),
            ctr: 0,
        },
        Receiver {
            r,
            aead: ChaCha20Poly1305::new(Key::from_slice(&recv)),
            ctr: 0,
        },
    )
}

impl<W: Write> Sender<W> {
    pub fn send(&mut self, plain: &[u8]) -> Result<(), String> {
        let n = nonce(self.ctr);
        self.ctr += 1;
        let ct = self
            .aead
            .encrypt(Nonce::from_slice(&n), plain)
            .map_err(|_| "encrypt failed".to_owned())?;
        write_frame(&mut self.w, &ct)
    }
}

impl<R: Read> Receiver<R> {
    pub fn recv(&mut self) -> Result<Vec<u8>, String> {
        let ct = read_frame(&mut self.r)?;
        let n = nonce(self.ctr);
        self.ctr += 1;
        self.aead
            .decrypt(Nonce::from_slice(&n), ct.as_slice())
            .map_err(|_| "auth failed (not paired or tampered)".to_owned())
    }
}

pub fn write_frame<W: Write>(w: &mut W, data: &[u8]) -> Result<(), String> {
    let len = u32::try_from(data.len()).map_err(|_| "frame too big".to_owned())?;
    w.write_all(&len.to_be_bytes())
        .and_then(|_| w.write_all(data))
        .and_then(|_| w.flush())
        .map_err(|e| e.to_string())
}

pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>, String> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).map_err(|e| e.to_string())?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(format!("frame of {len} bytes refused"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_code_gives_same_secret_wrong_code_does_not() {
        let (a, ma) = pair_start("123456");
        let (b, mb) = pair_start("123456");
        let ka = pair_finish(a, &mb).unwrap();
        let kb = pair_finish(b, &ma).unwrap();
        assert_eq!(pair_secret(&ka, "A", "B"), pair_secret(&kb, "B", "A"));

        let (c, mc) = pair_start("123456");
        let (d, md) = pair_start("654321");
        let kc = pair_finish(c, &md).unwrap();
        let kd = pair_finish(d, &mc).unwrap();
        assert_ne!(confirm_tag(&kc, "A"), confirm_tag(&kd, "A"));
    }

    #[cfg(unix)]
    #[test]
    fn channel_round_trip_and_wrong_secret_fails() {
        use std::os::unix::net::UnixStream;
        let (s1, s2) = UnixStream::pair().unwrap();
        let (mut c_tx, mut c_rx) =
            channel(s1.try_clone().unwrap(), s1, &[7u8; 32], "nc", "ns", true);
        let (mut s_tx, mut s_rx) =
            channel(s2.try_clone().unwrap(), s2, &[7u8; 32], "nc", "ns", false);
        c_tx.send(b"hello").unwrap();
        c_tx.send(b"again").unwrap();
        assert_eq!(s_rx.recv().unwrap(), b"hello");
        assert_eq!(s_rx.recv().unwrap(), b"again");
        s_tx.send(b"back").unwrap();
        assert_eq!(c_rx.recv().unwrap(), b"back");

        let (s3, s4) = UnixStream::pair().unwrap();
        let (mut good, _) = channel(s3.try_clone().unwrap(), s3, &[7u8; 32], "nc", "ns", true);
        let (_, mut bad) = channel(s4.try_clone().unwrap(), s4, &[8u8; 32], "nc", "ns", false);
        good.send(b"secret").unwrap();
        assert!(bad.recv().is_err());
    }

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..50 {
            let c = random_code();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
    }
}
