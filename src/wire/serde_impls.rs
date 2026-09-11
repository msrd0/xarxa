//! Implement serde for some types.
//!
//! Whenever possible, the (de)serialization should behave identical to those of the
//! equivalent types in `core`.

#[cfg(feature = "ipv4")]
use core::net::Ipv4Addr;
#[cfg(feature = "ipv6")]
use core::net::Ipv6Addr;
use core::{
    fmt::{self, Display, Formatter, Write as _},
    marker::PhantomData,
    str::FromStr,
};

use serde::de::{self, Deserialize, Deserializer, Unexpected, VariantAccess as _, Visitor};
use serde::ser::{Serialize, Serializer};

#[cfg(feature = "ipv4")]
use super::Ipv4Cidr;
#[cfg(feature = "ipv6")]
use super::Ipv6Cidr;
use super::{IpAddress as IpAddr, IpCidr, IpEndpoint, IpVersion};

struct FromStrVisitor<T> {
    expecting: &'static str,
    ty: PhantomData<T>,
}

impl<T> FromStrVisitor<T> {
    fn new(expecting: &'static str) -> Self {
        FromStrVisitor {
            expecting,
            ty: PhantomData,
        }
    }
}

impl<'de, T> Visitor<'de> for FromStrVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        s.parse().map_err(E::custom)
    }
}

static IP_VERSIONS: &[&str] = &[
    #[cfg(feature = "ipv4")]
    "V4",
    #[cfg(feature = "ipv6")]
    "V6",
];

struct IpVersionVisitor;

impl<'de> Visitor<'de> for IpVersionVisitor {
    type Value = IpVersion;

    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        #[cfg(all(feature = "ipv4", feature = "ipv6"))]
        return f.write_str("`V4` or `V6`");

        #[cfg(all(feature = "ipv4", not(feature = "ipv6")))]
        return f.write_str("`V4`");

        #[cfg(all(not(feature = "ipv4"), feature = "ipv6"))]
        return f.write_str("`V6`");
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value {
            #[cfg(feature = "ipv4")]
            0 => Ok(IpVersion::Ipv4),
            #[cfg(feature = "ipv6")]
            1 => Ok(IpVersion::Ipv6),
            _ => Err(E::invalid_value(Unexpected::Unsigned(value), &self)),
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value {
            #[cfg(feature = "ipv4")]
            "V4" => Ok(IpVersion::Ipv4),
            #[cfg(feature = "ipv6")]
            "V6" => Ok(IpVersion::Ipv6),
            _ => Err(E::unknown_variant(value, IP_VERSIONS)),
        }
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value {
            #[cfg(feature = "ipv4")]
            b"V4" => Ok(IpVersion::Ipv4),
            #[cfg(feature = "ipv6")]
            b"V6" => Ok(IpVersion::Ipv6),
            _ => match str::from_utf8(value) {
                Ok(value) => Err(E::unknown_variant(value, IP_VERSIONS)),
                Err(_) => Err(E::invalid_value(Unexpected::Bytes(value), &self)),
            },
        }
    }
}

impl<'de> Deserialize<'de> for IpVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_identifier(IpVersionVisitor)
    }
}

/// Deserialize an enum that has IPv4 / IPv6 variants based on the enabled features.
macro_rules! deserialize_enum {
    ($name:ident, $deserializer:ident) => {{
        struct EnumVisitor;

        impl<'de> Visitor<'de> for EnumVisitor {
            type Value = $name;

            fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_str(concat!("a ", stringify!($name)))
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: de::EnumAccess<'de>,
            {
                match data.variant()? {
                    #[cfg(feature = "ipv4")]
                    (IpVersion::Ipv4, v) => v.newtype_variant().map($name::Ipv4),
                    #[cfg(feature = "ipv6")]
                    (IpVersion::Ipv6, v) => v.newtype_variant().map($name::Ipv6),
                }
            }
        }

        $deserializer.deserialize_enum(stringify!($name), IP_VERSIONS, EnumVisitor)
    }};
}

/// Serialize a type using its [`Display`] implementation without allocations.
fn serialize_display_heapless<T, S, const N: usize>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    let mut buf = heapless::String::<N>::new();
    write!(&mut buf, "{value}").unwrap();
    serializer.serialize_str(&buf)
}

impl<'de> Deserialize<'de> for IpAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("IP address"))
        } else {
            deserialize_enum! { IpAddr, deserializer }
        }
    }
}

impl Serialize for IpAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            match self {
                #[cfg(feature = "ipv4")]
                Self::Ipv4(a) => a.serialize(serializer),
                #[cfg(feature = "ipv6")]
                Self::Ipv6(a) => a.serialize(serializer),
            }
        } else {
            match self {
                #[cfg(feature = "ipv4")]
                Self::Ipv4(a) => serializer.serialize_newtype_variant("IpAddr", 0, "V4", a),
                #[cfg(feature = "ipv6")]
                Self::Ipv6(a) => serializer.serialize_newtype_variant("IpAddr", 1, "V6", a),
            }
        }
    }
}

#[cfg(feature = "ipv4")]
impl<'de> Deserialize<'de> for Ipv4Cidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("IPv4 CIDR"))
        } else {
            <(_, u8)>::deserialize(deserializer).map(|(ip, prefix)| Self::new(ip, prefix))
        }
    }
}

#[cfg(feature = "ipv4")]
impl Serialize for Ipv4Cidr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            const MAX_LEN: usize = 19;
            debug_assert_eq!("101.102.103.104/250".len(), MAX_LEN);
            serialize_display_heapless::<_, _, MAX_LEN>(self, serializer)
        } else {
            (self.address(), self.prefix_len()).serialize(serializer)
        }
    }
}

#[cfg(feature = "ipv6")]
impl<'de> Deserialize<'de> for Ipv6Cidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("IPv6 CIDR"))
        } else {
            <(_, u8)>::deserialize(deserializer).map(|(ip, prefix)| Self::new(ip, prefix))
        }
    }
}

#[cfg(feature = "ipv6")]
impl Serialize for Ipv6Cidr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            const MAX_LEN: usize = 43;
            debug_assert_eq!("1001:1002:1003:1004:1005:1006:1007:1008/250".len(), MAX_LEN);
            serialize_display_heapless::<_, _, MAX_LEN>(self, serializer)
        } else {
            (self.address(), self.prefix_len()).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for IpCidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("IP CIDR"))
        } else {
            deserialize_enum! { IpCidr, deserializer }
        }
    }
}

impl Serialize for IpCidr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            match self {
                #[cfg(feature = "ipv4")]
                Self::Ipv4(a) => a.serialize(serializer),
                #[cfg(feature = "ipv6")]
                Self::Ipv6(a) => a.serialize(serializer),
            }
        } else {
            match self {
                #[cfg(feature = "ipv4")]
                Self::Ipv4(a) => serializer.serialize_newtype_variant("IpCidr", 0, "V4", a),
                #[cfg(feature = "ipv6")]
                Self::Ipv6(a) => serializer.serialize_newtype_variant("IpCidr", 1, "V6", a),
            }
        }
    }
}

enum SocketAddr {
    #[cfg(feature = "ipv4")]
    Ipv4((Ipv4Addr, u16)),
    #[cfg(feature = "ipv6")]
    Ipv6((Ipv6Addr, u16)),
}

impl<'de> Deserialize<'de> for IpEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("socket address"))
        } else {
            let socket_addr = deserialize_enum! { SocketAddr, deserializer }?;
            Ok(match socket_addr {
                #[cfg(feature = "ipv4")]
                SocketAddr::Ipv4((a, port)) => IpEndpoint::new(a.into(), port),
                #[cfg(feature = "ipv6")]
                SocketAddr::Ipv6((a, port)) => IpEndpoint::new(a.into(), port),
            })
        }
    }
}

impl Serialize for IpEndpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            const MAX_LEN: usize = 47;
            debug_assert_eq!("[1001:1002:1003:1004:1005:1006:1007:1008]:65000".len(), MAX_LEN);
            serialize_display_heapless::<_, _, MAX_LEN>(self, serializer)
        } else {
            match self.addr {
                #[cfg(feature = "ipv4")]
                IpAddr::Ipv4(a) => serializer.serialize_newtype_variant("SocketAddr", 0, "V4", &(a, self.port)),
                #[cfg(feature = "ipv6")]
                IpAddr::Ipv6(a) => serializer.serialize_newtype_variant("SocketAddr", 1, "V6", &(a, self.port)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Debug;
    use core::net::{Ipv4Addr, Ipv6Addr};
    use paste::paste;

    // Helper function to work around type inference
    #[track_caller]
    fn deser_using<T, U, F, E>(_: T, input: U, f: F) -> T
    where
        F: FnOnce(U) -> Result<T, E>,
        E: Debug,
    {
        f(input).unwrap()
    }

    macro_rules! serde_test {
        (name: $name:ident, value: $value:expr, json: $json:expr, cbor: $cbor:expr) => {
            paste! {
                #[test]
                fn [<test_ $name _deserialize_json>]() {
                    assert_eq!($value, deser_using($value, $json, serde_json::from_str));
                }

                #[test]
                fn [<test_ $name _serialize_json>]() {
                    assert_eq!($json, serde_json::to_string(&$value).unwrap());
                }

                #[test]
                fn [<test_ $name _deserialize_cbor>]() {
                    assert_eq!($value, deser_using($value, $cbor, cbor4ii::serde::from_slice));
                }

                #[test]
                fn [<test_ $name _serialize_cbor>]() {
                    assert_eq!($cbor, cbor4ii::serde::to_vec(Vec::new(), &$value).unwrap());
                }
            }
        };
    }

    // The following tests test both JSON (a human-readable format) and CBOR (a binary format)
    // against xarxa types and, if applicable, the equivalent type from std. This way we
    // know that the serde format of xarxa and std types is identical.

    const IPV4_JSON: &str = "\"127.0.0.1\"";
    const IPV4_CBOR: &[u8] = &[0xA1, 0x62, b'V', b'4', 0x84, 0x18, 127, 0, 0, 1];
    #[cfg(feature = "ipv4")]
    serde_test! {
        name: xarxa_ipv4_addr,
        value: IpAddr::Ipv4(Ipv4Addr::LOCALHOST),
        json: IPV4_JSON,
        cbor: IPV4_CBOR
    }
    serde_test! {
        name: std_ipv4_addr,
        value: std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
        json: IPV4_JSON,
        cbor: IPV4_CBOR
    }

    const IPV6_JSON: &str = "\"::1\"";
    const IPV6_CBOR: &[u8] = &[
        0xA1, 0x62, b'V', b'6', 0x90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    ];
    #[cfg(feature = "ipv6")]
    serde_test! {
        name: xarxa_ipv6_addr,
        value: IpAddr::Ipv6(Ipv6Addr::LOCALHOST),
        json: IPV6_JSON,
        cbor: IPV6_CBOR
    }
    serde_test! {
        name: std_ipv6_addr,
        value: std::net::IpAddr::V6(Ipv6Addr::LOCALHOST),
        json: IPV6_JSON,
        cbor: IPV6_CBOR
    }

    #[cfg(feature = "ipv4")]
    const IPV4_CIDR_JSON: &str = "\"127.0.0.1/8\"";
    #[cfg(feature = "ipv4")]
    const IPV4_CIDR_CBOR: &[u8] = &[0xA1, 0x62, b'V', b'4', 0x82, 0x84, 0x18, 127, 0, 0, 1, 8];
    #[cfg(feature = "ipv4")]
    serde_test! {
        name: xarxa_ipv4_cidr,
        value: IpCidr::Ipv4(Ipv4Cidr::new(Ipv4Addr::LOCALHOST, 8)),
        json: IPV4_CIDR_JSON,
        cbor: IPV4_CIDR_CBOR
    }

    #[cfg(feature = "ipv6")]
    const IPV6_CIDR_JSON: &str = "\"::1/128\"";
    #[cfg(feature = "ipv6")]
    const IPV6_CIDR_CBOR: &[u8] = &[
        0xA1, 0x62, b'V', b'6', 0x82, 0x90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x18, 128,
    ];
    #[cfg(feature = "ipv6")]
    serde_test! {
        name: xarxa_ipv6_cidr,
        value: IpCidr::Ipv6(Ipv6Cidr::new(Ipv6Addr::LOCALHOST, 128)),
        json: IPV6_CIDR_JSON,
        cbor: IPV6_CIDR_CBOR
    }

    const IPV4_ENDPOINT_JSON: &str = "\"127.0.0.1:80\"";
    const IPV4_ENDPOINT_CBOR: &[u8] = &[0xA1, 0x62, b'V', b'4', 0x82, 0x84, 0x18, 127, 0, 0, 1, 0x18, 80];
    #[cfg(feature = "ipv4")]
    serde_test! {
        name: xarxa_ipv4_endpoint,
        value: IpEndpoint::new(IpAddr::Ipv4(Ipv4Addr::LOCALHOST), 80),
        json: IPV4_ENDPOINT_JSON,
        cbor: IPV4_ENDPOINT_CBOR
    }
    serde_test! {
        name: std_ipv4_endpoint,
        value: std::net::SocketAddr::V4(std::net::SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80)),
        json: IPV4_ENDPOINT_JSON,
        cbor: IPV4_ENDPOINT_CBOR
    }

    const IPV6_ENDPOINT_JSON: &str = "\"[::1]:80\"";
    const IPV6_ENDPOINT_CBOR: &[u8] = &[
        0xA1, 0x62, b'V', b'6', 0x82, 0x90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x18, 80,
    ];
    #[cfg(feature = "ipv6")]
    serde_test! {
        name: xarxa_ipv6_endpoint,
        value: IpEndpoint::new(IpAddr::Ipv6(Ipv6Addr::LOCALHOST), 80),
        json: IPV6_ENDPOINT_JSON,
        cbor: IPV6_ENDPOINT_CBOR
    }
    serde_test! {
        name: std_ipv6_endpoint,
        value: std::net::SocketAddr::V6(std::net::SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0)),
        json: IPV6_ENDPOINT_JSON,
        cbor: IPV6_ENDPOINT_CBOR
    }
}
