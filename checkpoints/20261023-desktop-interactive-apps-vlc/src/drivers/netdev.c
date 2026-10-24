#include "netdev.h"
#include "string.h"
#include "serial.h"
#include "purple_detect.h"

static netdev_t *registered_devs[NETDEV_MAX];
static uint32_t netdev_count = 0;

int netdev_init(void) {
    memset(registered_devs, 0, sizeof(registered_devs));
    netdev_count = 0;
    return 0;
}

int netdev_register(netdev_t *dev) {
    if (!dev) return -1;
    if (netdev_count >= NETDEV_MAX) return -2;

    for (uint32_t i = 0; i < netdev_count; i++) {
        if (strcmp(registered_devs[i]->name, dev->name) == 0) {
            return -3; /* Already exists */
        }
    }

    registered_devs[netdev_count++] = dev;
    dev->flags |= NETDEV_FLAG_UP | NETDEV_FLAG_RUNNING;

    if (dev->ip_addr[0] == 0 && dev->ip_addr[1] == 0) {
        dev->ip_addr[0] = 10;
        dev->ip_addr[1] = 0;
        dev->ip_addr[2] = 2;
        dev->ip_addr[3] = 15; /* Standard QEMU default IP */
    }

    kinfo("NetDev: Registered '%s' (MAC %02x:%02x:%02x:%02x:%02x:%02x, IP %u.%u.%u.%u, MTU %u)",
          dev->name,
          dev->mac_addr[0], dev->mac_addr[1], dev->mac_addr[2],
          dev->mac_addr[3], dev->mac_addr[4], dev->mac_addr[5],
          dev->ip_addr[0], dev->ip_addr[1], dev->ip_addr[2], dev->ip_addr[3],
          dev->mtu);

    /* Notify PurpleDetect */
    purple_detect_register(PURPLE_CAT_NETWORK, dev->name, "Network Interface Adapter",
                           "AegisNet", "PCI", 0, 0, PURPLE_STATE_ACTIVE, 0);

    return 0;
}

int netdev_unregister(netdev_t *dev) {
    if (!dev) return -1;
    int found_idx = -1;
    for (uint32_t i = 0; i < netdev_count; i++) {
        if (registered_devs[i] == dev) {
            found_idx = (int)i;
            break;
        }
    }
    if (found_idx < 0) return -2;

    const purple_device_t *pd = purple_detect_find_by_name(dev->name);
    if (pd) {
        purple_detect_unregister(pd->id);
    }

    for (uint32_t i = found_idx; i + 1 < netdev_count; i++) {
        registered_devs[i] = registered_devs[i + 1];
    }
    netdev_count--;
    registered_devs[netdev_count] = NULL;
    return 0;
}

uint32_t netdev_get_count(void) {
    return netdev_count;
}

netdev_t *netdev_get(uint32_t index) {
    if (index >= netdev_count) return NULL;
    return registered_devs[index];
}

netdev_t *netdev_find_by_name(const char *name) {
    if (!name) return NULL;
    for (uint32_t i = 0; i < netdev_count; i++) {
        if (strcmp(registered_devs[i]->name, name) == 0) {
            return registered_devs[i];
        }
    }
    return NULL;
}

void netdev_dump(void) {
    kprintf("\n=== Network Interface Configuration (%u interface(s)) ===\n", netdev_count);
    if (netdev_count == 0) {
        kprintf("  No network interfaces registered.\n\n");
        return;
    }

    for (uint32_t i = 0; i < netdev_count; i++) {
        netdev_t *d = registered_devs[i];
        kprintf("  %s: flags=0x%x<UP,RUNNING> mtu %u\n", d->name, d->flags, d->mtu);
        kprintf("        inet %u.%u.%u.%u\n",
                d->ip_addr[0], d->ip_addr[1], d->ip_addr[2], d->ip_addr[3]);
        kprintf("        ether %02x:%02x:%02x:%02x:%02x:%02x\n",
                d->mac_addr[0], d->mac_addr[1], d->mac_addr[2],
                d->mac_addr[3], d->mac_addr[4], d->mac_addr[5]);
        kprintf("        RX packets %u  bytes %u\n", d->rx_packets, d->rx_bytes);
        kprintf("        TX packets %u  bytes %u\n\n", d->tx_packets, d->tx_bytes);
    }
}

void netdev_set_ip(netdev_t *dev, uint8_t a, uint8_t b, uint8_t c, uint8_t d) {
    if (!dev) return;
    dev->ip_addr[0] = a;
    dev->ip_addr[1] = b;
    dev->ip_addr[2] = c;
    dev->ip_addr[3] = d;
}

uint16_t net_htons(uint16_t v) {
    return (uint16_t)(((v & 0xFF) << 8) | ((v >> 8) & 0xFF));
}

uint16_t net_ntohs(uint16_t v) {
    return net_htons(v);
}

uint32_t net_htonl(uint32_t v) {
    return ((v & 0x000000FFU) << 24) |
           ((v & 0x0000FF00U) << 8)  |
           ((v & 0x00FF0000U) >> 8)  |
           ((v & 0xFF000000U) >> 24);
}

uint32_t net_ntohl(uint32_t v) {
    return net_htonl(v);
}

uint16_t net_checksum(const void *buf, size_t len) {
    const uint16_t *p = (const uint16_t *)buf;
    uint32_t sum = 0;
    while (len > 1) {
        sum += *p++;
        len -= 2;
    }
    if (len == 1) {
        sum += *(const uint8_t *)p;
    }
    while (sum >> 16) {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    return (uint16_t)(~sum);
}

int netdev_process_rx_packet(netdev_t *dev, const void *packet, uint32_t len) {
    if (!dev || !packet || len < sizeof(eth_header_t)) return -1;

    const eth_header_t *eth = (const eth_header_t *)packet;
    uint16_t ethertype = net_ntohs(eth->ethertype);

    /* 1. ARP Handling */
    if (ethertype == ETHERTYPE_ARP && len >= sizeof(eth_header_t) + sizeof(arp_packet_t)) {
        const arp_packet_t *arp = (const arp_packet_t *)((const uint8_t *)packet + sizeof(eth_header_t));
        uint16_t op = net_ntohs(arp->opcode);

        if (op == ARP_OP_REQUEST && memcmp(arp->target_ip, dev->ip_addr, 4) == 0) {
            uint8_t reply_buf[sizeof(eth_header_t) + sizeof(arp_packet_t)];
            eth_header_t *r_eth = (eth_header_t *)reply_buf;
            arp_packet_t *r_arp = (arp_packet_t *)(reply_buf + sizeof(eth_header_t));

            memcpy(r_eth->dest_mac, eth->src_mac, ETH_ALEN);
            memcpy(r_eth->src_mac, dev->mac_addr, ETH_ALEN);
            r_eth->ethertype = net_htons(ETHERTYPE_ARP);

            r_arp->hw_type = net_htons(1);
            r_arp->proto_type = net_htons(ETHERTYPE_IPV4);
            r_arp->hw_len = ETH_ALEN;
            r_arp->proto_len = 4;
            r_arp->opcode = net_htons(ARP_OP_REPLY);
            memcpy(r_arp->sender_mac, dev->mac_addr, ETH_ALEN);
            memcpy(r_arp->sender_ip, dev->ip_addr, 4);
            memcpy(r_arp->target_mac, arp->sender_mac, ETH_ALEN);
            memcpy(r_arp->target_ip, arp->sender_ip, 4);

            if (dev->transmit) {
                dev->transmit(dev, reply_buf, sizeof(reply_buf));
            }
            return 1;
        }
    }

    /* 2. IPv4 Handling */
    if (ethertype == ETHERTYPE_IPV4 && len >= sizeof(eth_header_t) + sizeof(ipv4_header_t)) {
        const ipv4_header_t *ip = (const ipv4_header_t *)((const uint8_t *)packet + sizeof(eth_header_t));

        if (memcmp(ip->dst_ip, dev->ip_addr, 4) == 0 && ip->protocol == IPV4_PROTO_ICMP) {
            uint32_t ip_hdr_len = (ip->version_ihl & 0x0F) * 4;
            if (len >= sizeof(eth_header_t) + ip_hdr_len + sizeof(icmp_header_t)) {
                const icmp_header_t *icmp = (const icmp_header_t *)((const uint8_t *)packet + sizeof(eth_header_t) + ip_hdr_len);
                if (icmp->type == ICMP_TYPE_ECHO_REQUEST) {
                    uint32_t total_payload = len - sizeof(eth_header_t) - ip_hdr_len;
                    uint8_t reply_buf[1514];
                    if (sizeof(eth_header_t) + sizeof(ipv4_header_t) + total_payload <= sizeof(reply_buf)) {
                        eth_header_t *r_eth = (eth_header_t *)reply_buf;
                        ipv4_header_t *r_ip = (ipv4_header_t *)(reply_buf + sizeof(eth_header_t));
                        icmp_header_t *r_icmp = (icmp_header_t *)(reply_buf + sizeof(eth_header_t) + sizeof(ipv4_header_t));

                        memcpy(r_eth->dest_mac, eth->src_mac, ETH_ALEN);
                        memcpy(r_eth->src_mac, dev->mac_addr, ETH_ALEN);
                        r_eth->ethertype = net_htons(ETHERTYPE_IPV4);

                        r_ip->version_ihl = 0x45;
                        r_ip->tos = 0;
                        r_ip->total_length = net_htons((uint16_t)(sizeof(ipv4_header_t) + total_payload));
                        r_ip->id = ip->id;
                        r_ip->flags_fragment = 0;
                        r_ip->ttl = 64;
                        r_ip->protocol = IPV4_PROTO_ICMP;
                        r_ip->checksum = 0;
                        memcpy(r_ip->src_ip, dev->ip_addr, 4);
                        memcpy(r_ip->dst_ip, ip->src_ip, 4);
                        r_ip->checksum = net_checksum(r_ip, sizeof(ipv4_header_t));

                        memcpy(r_icmp, icmp, total_payload);
                        r_icmp->type = ICMP_TYPE_ECHO_REPLY;
                        r_icmp->code = 0;
                        r_icmp->checksum = 0;
                        r_icmp->checksum = net_checksum(r_icmp, total_payload);

                        uint32_t reply_len = sizeof(eth_header_t) + sizeof(ipv4_header_t) + total_payload;
                        if (dev->transmit) {
                            dev->transmit(dev, reply_buf, reply_len);
                        }
                        return 2;
                    }
                }
            }
        }
    }

    return 0;
}
