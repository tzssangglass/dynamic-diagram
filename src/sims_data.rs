//! sims/ embedded at compile time: the binary is self-contained — any cwd,
//! any agent, no path resolution. sims/*.json stay the on-disk source of truth.
pub const SIMS: &[(&str, &str)] = &[
    ("anycast", include_str!("../sims/anycast.json")),
    ("arp", include_str!("../sims/arp.json")),
    ("bandwidth", include_str!("../sims/bandwidth.json")),
    ("bgp", include_str!("../sims/bgp.json")),
    ("certchain", include_str!("../sims/certchain.json")),
    ("checksum", include_str!("../sims/checksum.json")),
    ("dh", include_str!("../sims/dh.json")),
    ("dialup", include_str!("../sims/dialup.json")),
    ("dns", include_str!("../sims/dns.json")),
    ("encap", include_str!("../sims/encap.json")),
    ("igp", include_str!("../sims/igp.json")),
    ("ipbits", include_str!("../sims/ipbits.json")),
    ("linkclick", include_str!("../sims/linkclick.json")),
    ("modem", include_str!("../sims/modem.json")),
    ("msgjourney", include_str!("../sims/msgjourney.json")),
    ("mtu", include_str!("../sims/mtu.json")),
    ("nat", include_str!("../sims/nat.json")),
    ("netsim", include_str!("../sims/netsim.json")),
    ("quic", include_str!("../sims/quic.json")),
    ("routerhop", include_str!("../sims/routerhop.json")),
    ("switchlearn", include_str!("../sims/switchlearn.json")),
    ("tcphs", include_str!("../sims/tcphs.json")),
    ("tcpsim", include_str!("../sims/tcpsim.json")),
    ("tcpvsudp", include_str!("../sims/tcpvsudp.json")),
    ("telegraph", include_str!("../sims/telegraph.json")),
    ("tls", include_str!("../sims/tls.json")),
    ("vpn", include_str!("../sims/vpn.json")),
    ("wdm", include_str!("../sims/wdm.json")),
];

pub fn get(name: &str) -> Option<&'static str> {
    SIMS.iter().find(|(n, _)| *n == name).map(|(_, j)| *j)
}
