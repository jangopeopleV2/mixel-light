#[cfg(feature = "alloc")]
use alloc::{boxed::Box, format, sync::Arc, vec, vec::Vec};

use core::fmt;
use pki_types::PrivateKeyDer;
use ring::{rand::SystemRandom, rsa::KeyPair, signature};
use rustls::sign::{Signer, SigningKey};
use rustls::{SignatureAlgorithm, SignatureScheme};

const ALL_RSA_SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::RSA_PSS_SHA512,
    SignatureScheme::RSA_PSS_SHA384,
    SignatureScheme::RSA_PSS_SHA256,
    SignatureScheme::RSA_PKCS1_SHA512,
    SignatureScheme::RSA_PKCS1_SHA384,
    SignatureScheme::RSA_PKCS1_SHA256,
];

/// RSA signing through ring; no RustCrypto RSA private operation remains.
///
/// Imports use ring's validated two-prime PKCS#1 or unencrypted PKCS#8 v1
/// parsers. Their accepted modulus/exponent bounds are intentionally retained.
pub struct RsaSigningKey(Arc<KeyPair>);

impl TryFrom<&PrivateKeyDer<'_>> for RsaSigningKey {
    type Error = rustls::Error;

    fn try_from(value: &PrivateKeyDer<'_>) -> Result<Self, Self::Error> {
        let key = match value {
            PrivateKeyDer::Pkcs8(der) => KeyPair::from_pkcs8(der.secret_pkcs8_der()),
            PrivateKeyDer::Pkcs1(der) => KeyPair::from_der(der.secret_pkcs1_der()),
            _ => return Err(rustls::Error::General("not a supported RSA key format".into())),
        }
        .map_err(|error| rustls::Error::General(format!("failed to parse RSA private key: {error}")))?;
        Ok(Self(Arc::new(key)))
    }
}

impl SigningKey for RsaSigningKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let scheme = *ALL_RSA_SCHEMES
            .iter()
            .find(|scheme| offered.contains(scheme))?;
        let encoding: &'static dyn signature::RsaEncoding = match scheme {
            SignatureScheme::RSA_PSS_SHA512 => &signature::RSA_PSS_SHA512,
            SignatureScheme::RSA_PSS_SHA384 => &signature::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA256 => &signature::RSA_PSS_SHA256,
            SignatureScheme::RSA_PKCS1_SHA512 => &signature::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PKCS1_SHA384 => &signature::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA256 => &signature::RSA_PKCS1_SHA256,
            _ => return None,
        };
        Some(Box::new(RsaSigner {
            key: self.0.clone(),
            scheme,
            encoding,
        }))
    }

    fn algorithm(&self) -> SignatureAlgorithm {
        SignatureAlgorithm::RSA
    }
}

impl fmt::Debug for RsaSigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RsaSigningKey")
            .field("algorithm", &SignatureAlgorithm::RSA)
            .finish()
    }
}

struct RsaSigner {
    key: Arc<KeyPair>,
    scheme: SignatureScheme,
    encoding: &'static dyn signature::RsaEncoding,
}

impl Signer for RsaSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        let mut signature = vec![0; self.key.public().modulus_len()];
        self.key
            .sign(self.encoding, &SystemRandom::new(), message, &mut signature)
            .map_err(|_| rustls::Error::General("signing failed".into()))?;
        Ok(signature)
    }

    fn scheme(&self) -> SignatureScheme {
        self.scheme
    }
}

impl fmt::Debug for RsaSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RsaSigner")
            .field("scheme", &self.scheme)
            .finish()
    }
}
