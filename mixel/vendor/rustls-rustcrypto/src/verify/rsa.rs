use paste::paste;
use pki_types::{alg_id, AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm};

macro_rules! impl_generic_rsa_verifer {
    (
        $name:ident,
        $public_key_algo:expr,
        $signature_alg_id:expr,
        $parameters:expr
    ) => {
        paste! {
            #[allow(non_camel_case_types)]
            #[derive(Debug)]
            struct [<RsaVerifier_ $name>];

            impl SignatureVerificationAlgorithm for [<RsaVerifier_ $name>] {
                fn public_key_alg_id(&self) -> AlgorithmIdentifier {
                    $public_key_algo
                }

                fn signature_alg_id(&self) -> AlgorithmIdentifier {
                    $signature_alg_id
                }

                fn verify_signature(
                    &self,
                    public_key: &[u8],
                    message: &[u8],
                    signature: &[u8],
                ) -> Result<(), InvalidSignature> {
                    // Retain strict PKCS#1 DER parsing and the former rsa
                    // public-import ceiling. ring's generic verifier permits
                    // larger keys, which this adapter intentionally does not.
                    let parsed = pkcs1::RsaPublicKey::try_from(public_key)
                        .map_err(|_| InvalidSignature)?;
                    if parsed.modulus.as_bytes().len() > 512 {
                        return Err(InvalidSignature);
                    }
                    ring::signature::UnparsedPublicKey::new($parameters, public_key)
                        .verify(message, signature)
                        .map_err(|_| InvalidSignature)
                }
            }

            pub const $name: &dyn SignatureVerificationAlgorithm = &[<RsaVerifier_ $name>];
        }
    };
}

impl_generic_rsa_verifer!(
    RSA_PKCS1_SHA256,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PKCS1_SHA256,
    &ring::signature::RSA_PKCS1_2048_8192_SHA256
);
impl_generic_rsa_verifer!(
    RSA_PKCS1_SHA384,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PKCS1_SHA384,
    &ring::signature::RSA_PKCS1_2048_8192_SHA384
);
impl_generic_rsa_verifer!(
    RSA_PKCS1_SHA512,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PKCS1_SHA512,
    &ring::signature::RSA_PKCS1_2048_8192_SHA512
);

impl_generic_rsa_verifer!(
    RSA_PSS_SHA256,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PSS_SHA256,
    &ring::signature::RSA_PSS_2048_8192_SHA256
);
impl_generic_rsa_verifer!(
    RSA_PSS_SHA384,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PSS_SHA384,
    &ring::signature::RSA_PSS_2048_8192_SHA384
);
impl_generic_rsa_verifer!(
    RSA_PSS_SHA512,
    alg_id::RSA_ENCRYPTION,
    alg_id::RSA_PSS_SHA512,
    &ring::signature::RSA_PSS_2048_8192_SHA512
);
