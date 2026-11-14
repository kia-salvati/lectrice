//! "LAN only": both sides talk only to private and link-local addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// True for addresses on the home network: IPv4 10/8, 172.16/12, 192.168/16, 169.254/16; IPv6 fc00::/7, fe80::/10.
/// CGNAT 100.64/10 is refused, so VPN overlays never turn the link into "over the internet". Loopback is refused too
/// (a paired phone is never on this machine); tests opt in with [`is_allowed_peer`].
pub fn is_lan_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => is_lan_v4(v4),
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or_else(|| is_lan_v6(v6), is_lan_v4),
    }
}

/// [`is_lan_address`], plus loopback when `allow_loopback` (tests and the same-machine harness only).
pub fn is_allowed_peer(address: IpAddr, allow_loopback: bool) -> bool {
    is_lan_address(address) || (allow_loopback && address.to_canonical().is_loopback())
}

fn is_lan_v4(address: Ipv4Addr) -> bool {
    address.is_private() || address.is_link_local()
}

const UNIQUE_LOCAL_MASK: u16 = 0xfe00;
const UNIQUE_LOCAL_PREFIX: u16 = 0xfc00;
const LINK_LOCAL_MASK: u16 = 0xffc0;
const LINK_LOCAL_PREFIX: u16 = 0xfe80;

fn is_lan_v6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    first & UNIQUE_LOCAL_MASK == UNIQUE_LOCAL_PREFIX || first & LINK_LOCAL_MASK == LINK_LOCAL_PREFIX
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lan(text: &str) -> bool {
        is_lan_address(text.parse().unwrap())
    }

    #[test]
    fn accepts_home_network_addresses() {
        for address in ["10.1.2.3", "172.16.0.1", "172.31.255.254", "192.168.1.20", "169.254.10.10", "fd12::1", "fe80::1", "::ffff:192.168.1.5"] {
            assert!(lan(address), "{address}");
        }
    }

    #[test]
    fn refuses_public_and_overlay_addresses() {
        for address in
            ["8.8.8.8", "100.64.0.1", "100.127.255.1", "172.32.0.1", "192.169.0.1", "2001:db8::1", "2a00::1", "::ffff:8.8.8.8", "0.0.0.0", "127.0.0.1", "::1"]
        {
            assert!(!lan(address), "{address}");
        }
    }

    #[test]
    fn loopback_only_when_allowed() {
        for address in ["127.0.0.1", "::1", "::ffff:127.0.0.1"] {
            assert!(is_allowed_peer(address.parse().unwrap(), true));
            assert!(!is_allowed_peer(address.parse().unwrap(), false));
        }
    }
}
