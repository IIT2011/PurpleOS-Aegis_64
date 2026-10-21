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

    kinfo("NetDev: Registered '%s' (MAC %02x:%02x:%02x:%02x:%02x:%02x, MTU %u)",
          dev->name,
          dev->mac_addr[0], dev->mac_addr[1], dev->mac_addr[2],
          dev->mac_addr[3], dev->mac_addr[4], dev->mac_addr[5],
          dev->mtu);

    /* Notify PurpleDetect */
    purple_detect_register(DEVICE_CAT_NETWORK, dev->name, "Network Interface Adapter",
                           "AegisNet", DEVICE_BUS_PCI, 0, 0, DEVICE_STATE_ACTIVE);

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

    purple_device_t *pd = purple_detect_find_by_name(dev->name);
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
        kprintf("        ether %02x:%02x:%02x:%02x:%02x:%02x\n",
                d->mac_addr[0], d->mac_addr[1], d->mac_addr[2],
                d->mac_addr[3], d->mac_addr[4], d->mac_addr[5]);
        kprintf("        RX packets %u  bytes %u\n", d->rx_packets, d->rx_bytes);
        kprintf("        TX packets %u  bytes %u\n\n", d->tx_packets, d->tx_bytes);
    }
}
