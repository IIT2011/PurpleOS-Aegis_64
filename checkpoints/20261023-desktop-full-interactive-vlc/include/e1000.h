#ifndef E1000_H
#define E1000_H

#include "types.h"
#include "netdev.h"
#include "pci.h"

#define E1000_VENDOR_INTEL   0x8086
#define E1000_DEV_82540EM    0x100E
#define E1000_DEV_82545EM    0x100F
#define E1000_DEV_82543GC    0x1004
#define E1000_DEV_82574L     0x10D3

#define E1000_NUM_RX_DESC    16
#define E1000_NUM_TX_DESC    16
#define E1000_BUFFER_SIZE    2048

typedef struct {
    uint64_t addr;
    uint16_t length;
    uint16_t checksum;
    uint8_t  status;
    uint8_t  errors;
    uint16_t special;
} __attribute__((packed)) e1000_rx_desc_t;

typedef struct {
    uint64_t addr;
    uint16_t length;
    uint8_t  cso;
    uint8_t  cmd;
    uint8_t  status;
    uint8_t  css;
    uint16_t special;
} __attribute__((packed)) e1000_tx_desc_t;

typedef struct {
    const pci_device_t *pdev;
    uint64_t mmio_base;
    uint8_t  mac[6];
    int      link_up;
    uint32_t rx_cur;
    uint32_t tx_cur;
    e1000_rx_desc_t *rx_descs;
    e1000_tx_desc_t *tx_descs;
    void   **rx_buffers;
    void   **tx_buffers;
    netdev_t netdev;
} e1000_device_t;

int  e1000_init(void);
int  e1000_is_present(void);
const e1000_device_t *e1000_get_device(void);
int  e1000_transmit(netdev_t *dev, const void *buf, uint32_t len);
int  e1000_receive(netdev_t *dev, void *buf, uint32_t max_len);

#endif /* E1000_H */
