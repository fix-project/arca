#![no_std]

extern crate alloc;
#[cfg(target_os = "linux")]
extern crate std;

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use ssh_key::{Algorithm, PublicKey, SshSig};

#[cfg(target_os = "linux")]
pub mod linux;

pub const TAG_NAMESPACE: &str = "fix-tag-v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Policy {
    pub identities: Vec<String>,
    pub authorized_keys: Vec<String>,
    pub trusted_compilers: Vec<String>,
    pub trusted_taggers: Vec<String>,
    pub unsigned: bool,
}

#[derive(Debug)]
pub enum Error {
    Invalid,
    Untrusted,
    Signing(String),
    Configuration(String),
    Crypto(ssh_key::Error),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid certificate or trust configuration"),
            Self::Untrusted => f.write_str("certificate has no valid signature by a trusted key"),
            Self::Signing(error) => write!(f, "could not sign certificate: {error}"),
            Self::Configuration(error) => write!(f, "configuration error: {error}"),
            Self::Crypto(error) => error.fmt(f),
        }
    }
}
impl core::error::Error for Error {}
impl From<ssh_key::Error> for Error {
    fn from(error: ssh_key::Error) -> Self {
        Self::Crypto(error)
    }
}

pub fn public_key(text: &str) -> Result<PublicKey, Error> {
    let key = PublicKey::from_openssh(text)?;
    if key.algorithm() != Algorithm::Ed25519 {
        return Err(Error::Invalid);
    }
    Ok(key)
}

pub fn canonical_key(key: &PublicKey) -> Result<String, Error> {
    PublicKey::new(key.key_data().clone(), "")
        .to_openssh()
        .map_err(Error::from)
}

pub fn verify(
    namespace: &str,
    message: &[u8],
    signature: &str,
    keys: &[String],
) -> Result<(), Error> {
    let keys = keys
        .iter()
        .map(|text| public_key(text))
        .collect::<Result<Vec<_>, _>>()?;
    let signature = SshSig::from_pem(signature).map_err(|_| Error::Untrusted)?;
    if keys.iter().any(|key| {
        key.key_data() == signature.public_key()
            && key.verify(namespace, message, &signature).is_ok()
    }) {
        Ok(())
    } else {
        Err(Error::Untrusted)
    }
}

pub fn sign(
    namespace: &str,
    message: &[u8],
    identity: &str,
    mut signer: impl FnMut(&str, &str, &[u8]) -> Result<String, Error>,
) -> Result<String, Error> {
    let signature = signer(identity, namespace, message)?;
    let key = public_key(identity)?;
    key.verify(namespace, message, &SshSig::from_pem(&signature)?)?;
    Ok(signature)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub name: [u8; 32],
    pub signature: String,
}

impl Certificate {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        postcard::to_allocvec(self).map_err(|_| Error::Invalid)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        postcard::from_bytes(bytes).map_err(|_| Error::Invalid)
    }
    pub fn signer(&self) -> Result<String, Error> {
        let signature = SshSig::from_pem(&self.signature)?;
        PublicKey::new(signature.public_key().clone(), "")
            .to_openssh()
            .map_err(Error::from)
    }
    pub fn verify(&self, name: &[u8; 32], policy: &Policy) -> Result<(), Error> {
        if &self.name != name {
            return Err(Error::Invalid);
        }
        verify(
            TAG_NAMESPACE,
            name,
            &self.signature,
            &policy.authorized_keys,
        )
    }
}
