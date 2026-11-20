//! Pure Rust TCP/IP Network Stack for Aegis64
//!
//! Subsystems:
//!   - `eth`: Ethernet II framing (MAC address management, EtherType routing)
//!   - `arp`: Address Resolution Protocol (ARP table, request/reply generator)
//!   - `ipv4`: IPv4 packet processing, checksum computation, routing
//!   - `icmp`: Internet Control Message Protocol (Ping Echo Request / Reply)
//!   - `udp`: User Datagram Protocol
//!   - `tcp`: Transmission Control Protocol

pub mod arp;
pub mod dns;
pub mod eth;
mod https_roots;
pub mod icmp;
pub mod ipv4;
pub mod tcp;
pub mod udp;

use crate::sync::Spinlock;

pub const IPV4_DEFAULT_MTU: usize = 1500;
const RX_POLL_BUDGET: usize = 16;

#[derive(Clone, Copy)]
pub struct NetConfig {
    pub ip: [u8; 4],
    pub gateway: [u8; 4],
    pub subnet: [u8; 4],
    pub dns: Option<[u8; 4]>,
}

pub static NET_CONFIG: Spinlock<NetConfig> = Spinlock::new(NetConfig {
    ip: [0; 4],
    gateway: [0; 4],
    subnet: [0; 4],
    dns: None,
});

pub struct PendingPing {
    pub target_ip: [u8; 4],
    pub identifier: u16,
    pub seq_no: u16,
    pub received: bool,
    pub reply_time_ticks: u64,
}

static PENDING_PING: Spinlock<Option<PendingPing>> = Spinlock::new(None);
static PING_ACTIVE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static PING_SEQ: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(1);

struct PingGuard;

impl Drop for PingGuard {
    fn drop(&mut self) {
        *PENDING_PING.lock() = None;
        PING_ACTIVE.store(false, core::sync::atomic::Ordering::Release);
    }
}

pub fn init() {
    https_roots::init();
    crate::drivers::serial::println(
        "Network: Ethernet and IPv4 initialized; IPv4 configuration required",
    );
}

pub fn get_ip() -> [u8; 4] {
    NET_CONFIG.lock().ip
}

pub fn get_gateway() -> [u8; 4] {
    NET_CONFIG.lock().gateway
}

pub fn get_subnet() -> [u8; 4] {
    NET_CONFIG.lock().subnet
}

pub fn parse_ipv4(text: &str) -> Option<[u8; 4]> {
    let mut parts = text.split('.');
    let mut address = [0; 4];
    for octet in &mut address {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        *octet = part.parse().ok()?;
    }
    parts.next().is_none().then_some(address)
}

fn unicast(ip: [u8; 4]) -> bool {
    ip[0] != 0 && ip[0] != 127 && ip[0] < 224
}

pub fn set_config(ip: [u8; 4], gw: [u8; 4], subnet: [u8; 4]) -> Result<(), &'static str> {
    configure(ip, gw, subnet, None)
}

pub fn configure(
    ip: [u8; 4],
    gw: [u8; 4],
    subnet: [u8; 4],
    dns: Option<[u8; 4]>,
) -> Result<(), &'static str> {
    if dns.is_some_and(|server| !unicast(server) || server == ip) {
        return Err("Invalid DNS server address");
    }
    let mask = u32::from_be_bytes(subnet);
    let found = u32::from_be_bytes(ip);
    let gateway = u32::from_be_bytes(gw);
    let host = !mask;
    let valid_host = |address: u32| host <= 1 || (address & host != 0 && address & host != host);
    if mask == 0
        || mask.leading_ones() != mask.count_ones()
        || !unicast(ip)
        || !valid_host(found)
        || (gateway != 0
            && (!unicast(gw)
                || gateway == found
                || gateway & mask != found & mask
                || !valid_host(gateway)))
    {
        return Err("Invalid IPv4 address, subnet mask, or gateway");
    }
    let mut cfg = NET_CONFIG.lock();
    cfg.ip = ip;
    cfg.gateway = gw;
    cfg.subnet = subnet;
    cfg.dns = dns;
    arp::clear();
    Ok(())
}

pub fn get_my_mac() -> Option<[u8; 6]> {
    if crate::drivers::e1000::is_ready() {
        Some(crate::drivers::e1000::get_mac())
    } else if crate::drivers::rtl8139::is_ready() {
        Some(crate::drivers::rtl8139::get_mac())
    } else {
        None
    }
}

pub fn poll() {
    let mut rx_buf = [0u8; 2048];
    let receive = if crate::drivers::e1000::is_ready() {
        crate::drivers::e1000::recv
    } else if crate::drivers::rtl8139::is_ready() {
        crate::drivers::rtl8139::recv
    } else {
        return;
    };
    for _ in 0..RX_POLL_BUDGET {
        let len = receive(&mut rx_buf);
        if len == 0 {
            break;
        }
        process_packet(&rx_buf[..len]);
    }
}

pub fn process_packet(data: &[u8]) {
    let (eth_hdr, payload) = match eth::EthHeader::parse(data) {
        Some(res) => res,
        None => return,
    };
    let Some(my_mac) = get_my_mac() else { return };
    if eth_hdr.dst_mac != my_mac && eth_hdr.dst_mac != [0xff; 6] {
        return;
    }
    let my_ip = get_ip();
    if my_ip == [0; 4] {
        return;
    }

    match eth_hdr.ether_type {
        eth::ETHERTYPE_ARP => {
            if let Some(arp_pkt) = arp::ArpPacket::parse(payload) {
                if arp_pkt.target_ip != my_ip
                    || arp_pkt.sender_mac != eth_hdr.src_mac
                    || (arp_pkt.opcode == arp::ARP_OP_REPLY && arp_pkt.target_mac != my_mac)
                {
                    return;
                }
                if arp_pkt.opcode == arp::ARP_OP_REPLY
                    && unicast(arp_pkt.sender_ip)
                    && arp_pkt.sender_ip != my_ip
                {
                    arp::accept_reply(arp_pkt.sender_ip, arp_pkt.sender_mac);
                }

                if arp_pkt.opcode == arp::ARP_OP_REQUEST && arp_pkt.target_ip == my_ip {
                    // Send ARP reply
                    let reply_arp = arp::ArpPacket {
                        htype: 1,
                        ptype: eth::ETHERTYPE_IPV4,
                        hlen: 6,
                        plen: 4,
                        opcode: arp::ARP_OP_REPLY,
                        sender_mac: my_mac,
                        sender_ip: my_ip,
                        target_mac: arp_pkt.sender_mac,
                        target_ip: arp_pkt.sender_ip,
                    };

                    let reply_eth = eth::EthHeader {
                        dst_mac: arp_pkt.sender_mac,
                        src_mac: my_mac,
                        ether_type: eth::ETHERTYPE_ARP,
                    };

                    let mut pkt_buf = [0u8; 64];
                    let eth_len = reply_eth.write_to(&mut pkt_buf);
                    let arp_len = reply_arp.write_to(&mut pkt_buf[eth_len..]);
                    let total = eth_len + arp_len;
                    let _ = send_raw(&pkt_buf[..total]);
                }
            }
        }
        eth::ETHERTYPE_IPV4 => {
            if let Some((ip_hdr, ip_payload)) = ipv4::Ipv4Header::parse(payload) {
                if ip_hdr.dst_ip != my_ip
                    || eth_hdr.dst_mac != my_mac
                    || !unicast(ip_hdr.src_ip)
                    || eth_hdr.src_mac == [0; 6]
                    || eth_hdr.src_mac[0] & 1 != 0
                {
                    return;
                }

                if ip_hdr.protocol == ipv4::IP_PROTO_UDP {
                    dns::receive(ip_hdr.src_ip, my_ip, ip_payload);
                } else if ip_hdr.protocol == ipv4::IP_PROTO_ICMP {
                    if let Some((icmp_pkt, icmp_payload)) = icmp::IcmpEcho::parse(ip_payload) {
                        if icmp_pkt.msg_type == icmp::ICMP_TYPE_ECHO_REQUEST {
                            // Automatically answer ping directed at us
                            send_icmp_reply(
                                ip_hdr.src_ip,
                                eth_hdr.src_mac,
                                icmp_pkt.identifier,
                                icmp_pkt.seq_no,
                                icmp_payload,
                            );
                        } else if icmp_pkt.msg_type == icmp::ICMP_TYPE_ECHO_REPLY {
                            let mut pending = PENDING_PING.lock();
                            if let Some(ref mut p) = *pending {
                                if !p.received
                                    && p.target_ip == ip_hdr.src_ip
                                    && p.identifier == icmp_pkt.identifier
                                    && p.seq_no == icmp_pkt.seq_no
                                {
                                    p.received = true;
                                    p.reply_time_ticks = crate::drivers::pit::get_ticks();
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn send_icmp_reply(dst_ip: [u8; 4], dst_mac: [u8; 6], id: u16, seq: u16, payload: &[u8]) {
    let my_ip = get_ip();
    let my_mac = match get_my_mac() {
        Some(m) => m,
        None => return,
    };
    let mut buf = [0u8; eth::ETH_HEADER_LEN + IPV4_DEFAULT_MTU];
    let eth_hdr = eth::EthHeader {
        dst_mac,
        src_mac: my_mac,
        ether_type: eth::ETHERTYPE_IPV4,
    };
    let eth_len = eth_hdr.write_to(&mut buf);

    let icmp_reply = icmp::IcmpEcho {
        msg_type: icmp::ICMP_TYPE_ECHO_REPLY,
        code: 0,
        checksum: 0,
        identifier: id,
        seq_no: seq,
    };
    if buf.len() < eth_len + 20 {
        return;
    }
    let icmp_len = icmp_reply.write_to(payload, &mut buf[eth_len + 20..]);
    if icmp_len == 0 {
        return;
    }

    let ip_hdr = ipv4::Ipv4Header {
        version_ihl: ipv4::IPV4_VERSION_IHL_MIN,
        dscp_ecn: 0,
        total_len: (20 + icmp_len) as u16,
        id: seq,
        flags_frag: 0x4000,
        ttl: 64,
        protocol: ipv4::IP_PROTO_ICMP,
        checksum: 0,
        src_ip: my_ip,
        dst_ip,
    };
    let ip_len = ip_hdr.write_to(&mut buf[eth_len..eth_len + 20]);
    if ip_len == 0 {
        return;
    }

    let total = eth_len + ip_len + icmp_len;
    let _ = send_raw(&buf[..total]);
}

pub fn ping(target_ip: [u8; 4]) -> Result<u64, &'static str> {
    if PING_ACTIVE
        .compare_exchange(
            false,
            true,
            core::sync::atomic::Ordering::Acquire,
            core::sync::atomic::Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("Another ping is already in progress");
    }
    let _ping_guard = PingGuard;
    let config = *NET_CONFIG.lock();
    let my_ip = config.ip;
    if my_ip == [0; 4] {
        return Err("Configure an IPv4 address before sending packets");
    }
    if !unicast(target_ip) || target_ip == my_ip {
        return Err("Loopback routing is not implemented");
    }

    let my_mac = match get_my_mac() {
        Some(m) => m,
        None => return Err("No active Ethernet NIC online"),
    };

    let subnet = config.subnet;
    let gateway = config.gateway;

    let is_local = (target_ip[0] & subnet[0] == my_ip[0] & subnet[0])
        && (target_ip[1] & subnet[1] == my_ip[1] & subnet[1])
        && (target_ip[2] & subnet[2] == my_ip[2] & subnet[2])
        && (target_ip[3] & subnet[3] == my_ip[3] & subnet[3]);

    let next_hop = if is_local { target_ip } else { gateway };
    if next_hop == [0; 4] {
        return Err("No IPv4 route to destination");
    }

    // Resolve next-hop MAC via dynamic ARP cache or request
    let mut dst_mac_opt = arp::lookup_mac(next_hop);
    if dst_mac_opt.is_none() {
        arp::send_arp_request(next_hop)?;
        for _ in 0..60 {
            poll();
            if let Some(m) = arp::lookup_mac(next_hop) {
                dst_mac_opt = Some(m);
                break;
            }
            crate::drivers::pit::delay_ms(2);
        }
    }

    let dst_mac = match dst_mac_opt {
        Some(m) => m,
        None => return Err("Destination host unreachable (ARP resolution failed)"),
    };

    let ident: u16 = 0xAE64;
    let start_ticks = crate::drivers::pit::get_ticks();
    let seq = PING_SEQ.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    // Register pending ping before sending
    {
        let mut pending = PENDING_PING.lock();
        *pending = Some(PendingPing {
            target_ip,
            identifier: ident,
            seq_no: seq,
            received: false,
            reply_time_ticks: 0,
        });
    }

    let mut buf = [0u8; IPV4_DEFAULT_MTU];
    let eth_hdr = eth::EthHeader {
        dst_mac,
        src_mac: my_mac,
        ether_type: eth::ETHERTYPE_IPV4,
    };
    let eth_len = eth_hdr.write_to(&mut buf);

    let payload = b"Aegis64 Pure Rust ICMP Echo Ping";
    let icmp_echo = icmp::IcmpEcho {
        msg_type: icmp::ICMP_TYPE_ECHO_REQUEST,
        code: 0,
        checksum: 0,
        identifier: ident,
        seq_no: seq,
    };
    let mut icmp_buf = [0u8; 256];
    let icmp_len = icmp_echo.write_to(payload, &mut icmp_buf);

    let ip_hdr = ipv4::Ipv4Header {
        version_ihl: ipv4::IPV4_VERSION_IHL_MIN,
        dscp_ecn: 0,
        total_len: (20 + icmp_len) as u16,
        id: seq,
        flags_frag: 0x4000,
        ttl: 64,
        protocol: ipv4::IP_PROTO_ICMP,
        checksum: 0,
        src_ip: my_ip,
        dst_ip: target_ip,
    };
    let ip_len = ip_hdr.write_to(&mut buf[eth_len..]);
    buf[eth_len + ip_len..eth_len + ip_len + icmp_len].copy_from_slice(&icmp_buf[..icmp_len]);

    let total = (eth_len + ip_len + icmp_len).max(60);
    send_raw(&buf[..total])?;

    // Hardware poll loop waiting for actual ICMP Echo Reply
    let mut reply_received = false;
    let mut elapsed_ms: u64 = 0;

    for _ in 0..60 {
        poll();
        {
            let pending = PENDING_PING.lock();
            if let Some(ref p) = *pending {
                if p.received {
                    reply_received = true;
                    elapsed_ms = crate::drivers::pit::ticks_to_millis(
                        p.reply_time_ticks.saturating_sub(start_ticks),
                    )
                    .max(1);
                    break;
                }
            }
        }
        crate::drivers::pit::delay_ms(2);
    }

    if reply_received {
        Ok(elapsed_ms)
    } else {
        Err("Request timed out (no ICMP Echo Reply received)")
    }
}

pub fn send_raw(packet: &[u8]) -> Result<(), &'static str> {
    if crate::drivers::e1000::is_ready() {
        crate::drivers::e1000::send(packet)
    } else if crate::drivers::rtl8139::is_ready() {
        crate::drivers::rtl8139::send(packet)
    } else {
        Err("No network interface available for transmission")
    }
}
