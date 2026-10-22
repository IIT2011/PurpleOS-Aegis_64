#ifndef NETDEV_H
#define NETDEV_H

#include "types.h"

#define NETDEV_MAX 8
#define NETDEV_NAME_LEN 16
#define ETH_ALEN 6

#define NETDEV_FLAG_UP        0x0001
#define NETDEV_FLAG_BROADCAST 0x0002
#define NETDEV_FLAG_RUNNING   0x0004
#define NETDEV_FLAG_LOOPBACK  0x0008

#define ETHERTYPE_IPV4 0x0800
#define ETHERTYPE_ARP  0x0806

#define ARP_OP_REQUEST 1
#define ARP_OP_REPLY   2

#define IPV4_PROTO_ICMP 1
#define IPV4_PROTO_TCP  6
#define IPV4_PROTO_UDP  17

#define ICMP_TYPE_ECHO_REPLY   0
#define ICMP_TYPE_ECHO_REQUEST 8

typedef struct eth_header {
    uint8_t  dest_mac[ETH_ALEN];
    uint8_t  src_mac[ETH_ALEN];
    uint16_t ethertype;
} __attribute__((packed)) eth_header_t;

typedef struct arp_packet {
    uint16_t hw_type;
    uint16_t proto_type;
    uint8_t  hw_len;
    uint8_t  proto_len;
    uint16_t opcode;
    uint8_t  sender_mac[ETH_ALEN];
    uint8_t  sender_ip[4];
    uint8_t  target_mac[ETH_ALEN];
    uint8_t  target_ip[4];
} __attribute__((packed)) arp_packet_t;

typedef struct ipv4_header {
    uint8_t  version_ihl;
    uint8_t  tos;
    uint16_t total_length;
    uint16_t id;
    uint16_t flags_fragment;
    uint8_t  ttl;
    uint8_t  protocol;
    uint16_t checksum;
    uint8_t  src_ip[4];
    uint8_t  dst_ip[4];
} __attribute__((packed)) ipv4_header_t;

typedef struct icmp_header {
    uint8_t  type;
    uint8_t  code;
    uint16_t checksum;
    uint16_t id;
    uint16_t sequence;
} __attribute__((packed)) icmp_header_t;

typedef struct netdev {
    char name[NETDEV_NAME_LEN];
    uint8_t mac_addr[ETH_ALEN];
    uint8_t ip_addr[4];
    uint8_t netmask[4];
    uint8_t gateway[4];
    uint16_t mtu;
    uint32_t flags;
    uint32_t rx_packets;
    uint32_t tx_packets;
    uint32_t rx_bytes;
    uint32_t tx_bytes;
    int (*transmit)(struct netdev *dev, const void *buf, uint32_t len);
    int (*receive)(struct netdev *dev, void *buf, uint32_t max_len);
    void *priv;
} netdev_t;

int netdev_init(void);
int netdev_register(netdev_t *dev);
int netdev_unregister(netdev_t *dev);
uint32_t netdev_get_count(void);
netdev_t *netdev_get(uint32_t index);
netdev_t *netdev_find_by_name(const char *name);
void netdev_dump(void);
void netdev_set_ip(netdev_t *dev, uint8_t a, uint8_t b, uint8_t c, uint8_t d);
int netdev_process_rx_packet(netdev_t *dev, const void *packet, uint32_t len);

uint16_t net_htons(uint16_t v);
uint16_t net_ntohs(uint16_t v);
uint32_t net_htonl(uint32_t v);
uint32_t net_ntohl(uint32_t v);
uint16_t net_checksum(const void *buf, size_t len);

#endif /* NETDEV_H */
