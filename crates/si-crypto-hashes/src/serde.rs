use crate::{HashAlgorithm, HashDigest};

impl serde::Serialize for HashAlgorithm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.name())
    }
}

impl<'de> serde::Deserialize<'de> for HashAlgorithm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = HashAlgorithm;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("hash algorithm")
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                s.parse()
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(s), &"hash algorithm"))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl<D: AsRef<[u8]>> serde::Serialize for HashDigest<D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de, Data: From<Vec<u8>>> serde::Deserialize<'de> for HashDigest<Data> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<Data> {
            _marker: std::marker::PhantomData<Data>,
        }

        impl<Data: From<Vec<u8>>> serde::de::Visitor<'_> for Visitor<Data> {
            type Value = HashDigest<Data>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("hash digest")
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                s.parse()
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(s), &"hash digest"))
            }
        }

        deserializer.deserialize_str(Visitor {
            _marker: std::marker::PhantomData,
        })
    }
}
