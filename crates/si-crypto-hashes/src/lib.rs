#![cfg_attr(docsrs, feature(doc_auto_cfg))]
//! This crate provides a reusable functionality for working with typical cryptographic
//! hashes.
//!
//! ```rust
//! # use std::str::FromStr;
//! # use std::sync::Arc;
//! # use si_crypto_hashes::{HashAlgorithm, HashDigest};
//! #
//! let expected = "sha256_dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
//!
//! // Parse the string representation of the expected hash.
//! let digest = HashDigest::<Arc<[u8]>>::from_str(expected).unwrap();
//! assert_eq!(digest.algorithm(), HashAlgorithm::Sha256);
//! assert_eq!(digest.to_string(), expected);
//!
//! // Compute a digest.
//! let mut hasher = digest.algorithm().hasher();
//! hasher.update(b"Hello, World!");
//! assert_eq!(hasher.finalize(), digest);
//! ```
//!
//! For parsing, the string representation is expected to be in the format
//! `<algorithm>_<digest>` or `<algorithm>:<digest>`.
//!
//! The algorithm must be one of the following:
//!
//! - `sha256`
//! - `sha512_256` or `sha512-256`
//! - `sha512`
//!
//! Note that the underscore representation is preferred as it can be selected by double
//! clicking in most applications.
//!
//! In the future, we may add additional hash algorithms.
//!
//! # Features
//!
//! This crate supports the following features:
//!
//! - `serde`: Enables serialization and deserialization support using Serde.
//! - `legacy`: Use legacy formatting and algorithm names for compatibility with older
//!   parsers.

use std::fmt::Write;
use std::str::FromStr;
use std::sync::Arc;

use sha2::Digest;

#[cfg(feature = "serde")]
mod serde;

/// Define the data structures for the hash algorithms.
macro_rules! define_hash_algorithms {
    ($($variant:ident, $name:literal, [$($alias:literal),*], $size:literal, $hasher:ty;)*) => {
        /// Cryptographic hash algorithms.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum HashAlgorithm {
            $(
                $variant,
            )*
        }

        impl HashAlgorithm {
            /// Name of the algorithm.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(
                        Self::$variant => $name,
                    )*
                }
            }

            /// Create a fresh hasher.
            pub fn hasher(self) -> Hasher {
                match self {
                    $(
                        Self::$variant => Hasher {
                            algorithm: self,
                            inner: HasherInner::$variant(<$hasher>::new())
                        },
                    )*
                }
            }

            /// Size of the hash.
            #[must_use]
            pub const fn hash_size(self) -> usize {
                match self {
                    $(
                        Self::$variant => $size,
                    )*
                }
            }
        }

        impl FromStr for HashAlgorithm {
            type Err = InvalidAlgorithmError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $(
                        $name $(| $alias)* => Ok(Self::$variant),
                    )*
                    _ => Err(InvalidAlgorithmError),
                }
            }
        }

        /// Internal hasher representation.
        #[derive(Debug, Clone)]
        enum HasherInner {
            $(
                $variant($hasher),
            )*
        }

        impl HasherInner {
            /// Update the hash with the given bytes.
            fn update(&mut self, bytes: &[u8]) {
                match self {
                    $(
                        HasherInner::$variant(hasher) => hasher.update(bytes),
                    )*
                }
            }

            /// Finalize the hash.
            #[must_use]
            fn finalize<D>(self) -> D
            where
                D: for<'slice> From<&'slice [u8]>
            {
                match self {
                    $(
                        HasherInner::$variant(hasher) => hasher.finalize().as_slice().into(),
                    )*
                }
            }
        }
    };
}

define_hash_algorithms! {
    Sha256, "sha256", [], 32, sha2::Sha256;
    Sha512_256, "sha512_256", ["sha512-256"], 32, sha2::Sha512_256;
    Sha512, "sha512", [], 64, sha2::Sha512;
}

impl HashAlgorithm {
    /// Hash the given bytes.
    #[must_use]
    pub fn hash<D>(self, bytes: &[u8]) -> HashDigest<D>
    where
        D: for<'slice> From<&'slice [u8]>,
    {
        let mut hasher = self.hasher();
        hasher.update(bytes);
        hasher.finalize()
    }
}

/// Invalid hash algorithm error.
#[derive(Debug)]
pub struct InvalidAlgorithmError;

impl std::fmt::Display for InvalidAlgorithmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid hash algorithm")
    }
}

impl std::error::Error for InvalidAlgorithmError {}

/// Hasher for computing hashes.
#[derive(Debug, Clone)]
#[must_use]
pub struct Hasher {
    algorithm: HashAlgorithm,
    inner: HasherInner,
}

impl Hasher {
    /// Algorithm of the hasher.
    #[must_use]
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    /// Update the hasher with the given bytes.
    pub fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }

    /// Finalize the hasher and return the digest.
    #[must_use]
    pub fn finalize<D>(self) -> HashDigest<D>
    where
        D: for<'slice> From<&'slice [u8]>,
    {
        HashDigest {
            algorithm: self.algorithm,
            raw: self.inner.finalize(),
        }
    }
}

/// Hash digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HashDigest<D = Arc<[u8]>> {
    /// Algorithm used to compute the digest.
    algorithm: HashAlgorithm,
    /// Raw digest.
    raw: D,
}

impl<D> HashDigest<D>
where
    D: AsRef<[u8]>,
{
    /// Create [`HashDigest`] from the provided algorithm and raw digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the length of the raw digest does not match the expected size
    /// for the algorithm.
    pub fn new(algorithm: HashAlgorithm, raw: D) -> Result<Self, InvalidDigestError> {
        if raw.as_ref().len() != algorithm.hash_size() {
            return Err(InvalidDigestError("invalid digest size"));
        }
        Ok(Self::new_unchecked(algorithm, raw))
    }

    /// Create [`HashDigest`] from the provided algorithm and raw digest without checking
    /// the digest's length.
    pub const fn new_unchecked(algorithm: HashAlgorithm, raw: D) -> Self {
        Self { algorithm, raw }
    }

    /// Algorithm used to compute the digest.
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    /// Raw digest.
    pub fn raw(&self) -> &[u8] {
        self.raw.as_ref()
    }

    /// Convert the raw digest to a hex string.
    pub fn raw_hex_string(&self) -> String {
        hex::encode(&self.raw)
    }

    /// Convert the digest back to its raw representation.
    pub fn into_inner(self) -> D {
        self.raw
    }
}

impl<D> std::fmt::Display for HashDigest<D>
where
    D: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.algorithm {
            HashAlgorithm::Sha512_256 if cfg!(feature = "legacy") => f.write_str("sha512-256")?,
            _ => f.write_str(self.algorithm.name())?,
        };
        if cfg!(feature = "legacy") {
            f.write_char(':')?;
        } else {
            f.write_char('_')?;
        }
        f.write_str(&self.raw_hex_string())?;
        Ok(())
    }
}

impl<D> AsRef<[u8]> for HashDigest<D>
where
    D: AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        self.raw.as_ref()
    }
}

impl<D: From<Vec<u8>>> FromStr for HashDigest<D> {
    type Err = InvalidDigestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((algorithm, digest)) = s.rsplit_once([':', '_']) else {
            return Err(InvalidDigestError("missing delimiter, expected ':' or '_'"));
        };
        let algorithm = HashAlgorithm::from_str(algorithm)?;
        let Ok(raw) = hex::decode(digest) else {
            return Err(InvalidDigestError("digest is not a hex string"));
        };
        if raw.len() != algorithm.hash_size() {
            return Err(InvalidDigestError("invalid digest size"));
        }
        Ok(Self {
            algorithm,
            raw: raw.into(),
        })
    }
}

/// Invalid hash digest error.
#[derive(Debug)]
pub struct InvalidDigestError(pub(crate) &'static str);

impl std::fmt::Display for InvalidDigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for InvalidDigestError {}

impl From<InvalidAlgorithmError> for InvalidDigestError {
    fn from(_: InvalidAlgorithmError) -> Self {
        InvalidDigestError("invalid hash algorithm")
    }
}
