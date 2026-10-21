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

typedef struct netdev {
    char name[NETDEV_NAME_LEN];
    uint8_t mac_addr[ETH_ALEN];
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

#endif /* NETDEV_H */
