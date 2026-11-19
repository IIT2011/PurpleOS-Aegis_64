use crate::{arch::x86_64::cpu::cpuid, drivers::pit, sync::Spinlock};
use super::{arp, eth, ipv4, udp, NET_CONFIG};

const DNS_PORT: u16 = 53;
const HEADER: usize = 12;
const MAX_NAME: usize = 255;
const TIMEOUT_MS: u64 = 1500;
static QUERY: Spinlock<Option<Query>> = Spinlock::new(None);

struct Query {
    server: [u8; 4],
    port: u16,
    id: u16,
    name: [u8; MAX_NAME],
    len: usize,
    answer: Option<[u8; 4]>,
}

pub fn host(target: &str) -> &str {
    if let Some((scheme, rest)) = target.split_once("://") {
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            return rest.split(['/', '?', '#']).next().unwrap_or("");
        }
    }
    target
}

fn encode(host: &str, out: &mut [u8; MAX_NAME]) -> Option<usize> {
    let mut pos = 0;
    for label in host.strip_suffix('.').unwrap_or(host).split('.') {
        let len = label.len();
        if !(1..=63).contains(&len) || pos + len + 1 >= out.len()
            || label.starts_with('-') || label.ends_with('-')
            || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return None;
        }
        out[pos] = len as u8;
        out[pos + 1..pos + 1 + len].copy_from_slice(label.as_bytes());
        pos += len + 1;
    }
    out[pos] = 0;
    Some(pos + 1)
}

fn random() -> Option<u32> {
    if cpuid(0, 0).eax < 1 || cpuid(1, 0).ecx & (1 << 30) == 0 {
        return None;
    }
    for _ in 0..10 {
        let value: u32;
        let success: u8;
        unsafe {
            core::arch::asm!("rdrand {value:e}", "setc {success}", value = out(reg) value,
                success = out(reg_byte) success, options(nostack, nomem));
        }
        if success != 0 { return Some(value); }
    }
    None
}

pub fn resolve(host: &str, server: [u8; 4]) -> Result<[u8; 4], &'static str> {
    let mut name = [0; MAX_NAME];
    let len = encode(host, &mut name).ok_or("Invalid hostname or HTTP(S) address")?;
    let config = *NET_CONFIG.lock();
    let mac = super::get_my_mac().ok_or("No active Ethernet interface")?;
    if config.ip == [0; 4] { return Err("Configure IPv4 before resolving a hostname"); }
    if !super::unicast(server) || server == config.ip { return Err("Invalid DNS server address"); }
    let random = random().ok_or("Secure DNS query randomness unavailable")?;
    let id = random as u16;
    let port = 49152 + ((random >> 16) as u16 & 0x3fff);
    let mut pending = QUERY.lock();
    if pending.is_some() { return Err("A DNS query is already pending"); }
    *pending = Some(Query { server, port, id, name, len, answer: None });
    drop(pending);
    let result = exchange(config, mac, server, port, id, &name[..len]);
    *QUERY.lock() = None;
    result
}

fn exchange(config: super::NetConfig, mac: [u8; 6], server: [u8; 4], port: u16, id: u16, name: &[u8])
    -> Result<[u8; 4], &'static str>
{
    let mask = u32::from_be_bytes(config.subnet);
    let hop = if (u32::from_be_bytes(config.ip) ^ u32::from_be_bytes(server)) & mask == 0 {
        server
    } else { config.gateway };
    if hop == [0; 4] { return Err("No IPv4 route to the DNS server"); }
    if arp::lookup_mac(hop).is_none() { arp::send_arp_request(hop)?; }
    let start = pit::get_ticks();
    let destination = loop {
        super::poll();
        if let Some(mac) = arp::lookup_mac(hop) { break mac; }
        if elapsed(start) >= TIMEOUT_MS { return Err("DNS next-hop ARP timed out"); }
        pit::delay_ms(2);
    };

    let mut query = [0; HEADER + MAX_NAME + 4];
    query[..2].copy_from_slice(&id.to_be_bytes());
    query[2] = 1; // Recursion desired
    query[5] = 1; // One question
    let end = HEADER + name.len();
    query[HEADER..end].copy_from_slice(name);
    query[end..end + 4].copy_from_slice(&[0, 1, 0, 1]); // A, IN
    let mut frame = [0; 512];
    let eth_len = eth::EthHeader { dst_mac: destination, src_mac: mac, ether_type: eth::ETHERTYPE_IPV4 }
        .write_to(&mut frame);
    let ip_len = ipv4::IPV4_HEADER_MIN_LEN;
    let udp_len = udp::UdpHeader { src_port: port, dst_port: DNS_PORT, length: 0, checksum: 0 }
        .write_ipv4(&query[..end + 4], config.ip, server, &mut frame[eth_len + ip_len..]);
    ipv4::Ipv4Header {
        version_ihl: ipv4::IPV4_VERSION_IHL_MIN, dscp_ecn: 0,
        total_len: (ip_len + udp_len) as u16, id, flags_frag: 0x4000, ttl: 64,
        protocol: ipv4::IP_PROTO_UDP, checksum: 0, src_ip: config.ip, dst_ip: server,
    }.write_to(&mut frame[eth_len..eth_len + ip_len]);
    super::send_raw(&frame[..eth_len + ip_len + udp_len])?;
    let start = pit::get_ticks();
    loop {
        super::poll();
        if let Some(ip) = QUERY.lock().as_ref().and_then(|q| q.answer) { return Ok(ip); }
        if elapsed(start) >= TIMEOUT_MS { return Err("No usable IPv4 DNS reply"); }
        pit::delay_ms(2);
    }
}

fn elapsed(start: u64) -> u64 { pit::ticks_to_millis(pit::get_ticks().saturating_sub(start)) }

pub(super) fn receive(source: [u8; 4], destination: [u8; 4], packet: &[u8]) {
    let Some((udp, bytes)) = udp::UdpHeader::parse_ipv4(packet, source, destination) else { return };
    let mut pending = QUERY.lock();
    let Some(q) = pending.as_mut() else { return };
    if source == q.server && udp.src_port == DNS_PORT && udp.dst_port == q.port && q.answer.is_none() {
        q.answer = answer(bytes, q.id, &q.name[..q.len]);
    }
}

fn word(bytes: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(pos..pos + 2)?.try_into().ok()?))
}

fn name(bytes: &[u8], cursor: &mut usize, out: &mut [u8; MAX_NAME]) -> Option<usize> {
    let mut pos = *cursor;
    let mut written = 0;
    let mut jumped = false;
    for _ in 0..MAX_NAME {
        let len = usize::from(*bytes.get(pos)?);
        if len & 0xc0 == 0xc0 {
            let next = usize::from(word(bytes, pos)? & 0x3fff);
            if next < HEADER || next >= pos { return None; }
            if !jumped { *cursor = pos + 2; jumped = true; }
            pos = next;
            continue;
        }
        if len > 63 || written + len >= out.len() { return None; }
        out[written] = len as u8;
        written += 1;
        pos += 1;
        if len == 0 {
            if !jumped { *cursor = pos; }
            return Some(written);
        }
        out[written..written + len].copy_from_slice(bytes.get(pos..pos + len)?);
        written += len;
        pos += len;
    }
    None
}

fn answer(bytes: &[u8], id: u16, question: &[u8]) -> Option<[u8; 4]> {
    if word(bytes, 0)? != id || word(bytes, 2)? & 0xfa0f != 0x8000 || word(bytes, 4)? != 1 {
        return None;
    }
    let mut cursor = HEADER;
    let mut owner = [0; MAX_NAME];
    let qlen = name(bytes, &mut cursor, &mut owner)?;
    if !owner[..qlen].eq_ignore_ascii_case(question) || bytes.get(cursor..cursor + 4)? != [0, 1, 0, 1] {
        return None;
    }
    let start = cursor + 4;
    let mut expected = [0; MAX_NAME];
    expected[..qlen].copy_from_slice(question);
    let mut expected_len = qlen;
    for _ in 0..8 {
        cursor = start;
        let mut alias = None;
        for _ in 0..word(bytes, 6)? {
            let len = name(bytes, &mut cursor, &mut owner)?;
            let kind = word(bytes, cursor)?;
            let class = word(bytes, cursor + 2)?;
            let size = usize::from(word(bytes, cursor + 8)?);
            let data = cursor + 10;
            cursor = data + size;
            let payload = bytes.get(data..cursor)?;
            if class != 1 || !owner[..len].eq_ignore_ascii_case(&expected[..expected_len]) { continue; }
            if kind == 1 && size == 4 {
                let ip = payload.try_into().ok()?;
                if super::unicast(ip) { return Some(ip); }
            }
            if kind == 5 { alias = Some((data, cursor)); }
        }
        let (mut pos, end) = alias?;
        expected_len = name(bytes, &mut pos, &mut expected)?;
        if pos != end { return None; }
    }
    None
}
