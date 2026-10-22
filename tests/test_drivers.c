#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "rtc.h"
#include "usb.h"
#include "e1000.h"
#include "audio.h"
#include "netdev.h"
#include "pci.h"
#include "purple_detect.h"
#include "apdrv.h"
#include "ksym.h"

static void test_rtc_read_and_formatting(void) {
    printf("[TEST  1] Running test_rtc_read_and_formatting             ... ");
    assert(rtc_init() == 0);

    rtc_time_t t;
    memset(&t, 0, sizeof(t));
    rtc_get_time(&t);

    assert(t.year >= 2026);
    assert(t.month >= 1 && t.month <= 12);
    assert(t.day >= 1 && t.day <= 31);
    assert(t.hour < 24);
    assert(t.minute < 60);
    assert(t.second < 60);

    char buf[32];
    memset(buf, 0, sizeof(buf));
    rtc_format_time(buf, sizeof(buf));

    assert(strlen(buf) == 19);
    assert(buf[4] == '-' && buf[7] == '-');
    assert(buf[10] == ' ');
    assert(buf[13] == ':' && buf[16] == ':');

    printf("PASSED\n");
}

static void test_usb_controller_inventory(void) {
    printf("[TEST  2] Running test_usb_controller_inventory            ... ");
    purple_detect_init();
    pci_init();

    /* Register mock PCI USB host controllers */
    pci_device_t uhci_dev;
    memset(&uhci_dev, 0, sizeof(uhci_dev));
    uhci_dev.bus = 0; uhci_dev.dev = 1; uhci_dev.func = 2;
    uhci_dev.vendor_id = 0x8086; uhci_dev.device_id = 0x7020;
    uhci_dev.class_code = 0x0C; uhci_dev.subclass = 0x03; uhci_dev.prog_if = 0x00; /* UHCI */
    uhci_dev.bar[4] = 0x2001; /* I/O BAR */
    pci_register_device(&uhci_dev);

    pci_device_t xhci_dev;
    memset(&xhci_dev, 0, sizeof(xhci_dev));
    xhci_dev.bus = 0; xhci_dev.dev = 2; xhci_dev.func = 0;
    xhci_dev.vendor_id = 0x1B36; xhci_dev.device_id = 0x000D;
    xhci_dev.class_code = 0x0C; xhci_dev.subclass = 0x03; xhci_dev.prog_if = 0x30; /* xHCI */
    xhci_dev.bar[0] = 0xFEB00000; /* MMIO BAR */
    pci_register_device(&xhci_dev);

    assert(usb_init() == 0);
    assert(usb_get_controller_count() == 2);

    const usb_controller_t *c0 = usb_get_controller(0);
    assert(c0 != NULL);
    assert(c0->type == USB_TYPE_UHCI);
    assert(c0->io_base == 0x2000);
    assert(c0->max_ports == 2);

    const usb_controller_t *c1 = usb_get_controller(1);
    assert(c1 != NULL);
    assert(c1->type == USB_TYPE_XHCI);
    assert(c1->mmio_base == 0xFEB00000);
    assert(c1->max_ports >= 4);

    /* Verify PurpleDetect registration */
    assert(purple_detect_find_by_name("Intel UHCI Host") != NULL);
    assert(purple_detect_find_by_name("xHCI USB 3.0 Host") != NULL);

    printf("PASSED\n");
}

static void test_e1000_ethernet_operations(void) {
    printf("[TEST  3] Running test_e1000_ethernet_operations           ... ");
    netdev_init();

    /* Register mock Intel 82540EM PCI device */
    pci_device_t e1000_pci;
    memset(&e1000_pci, 0, sizeof(e1000_pci));
    e1000_pci.bus = 0; e1000_pci.dev = 3; e1000_pci.func = 0;
    e1000_pci.vendor_id = 0x8086; e1000_pci.device_id = 0x100E;
    e1000_pci.class_code = 0x02; e1000_pci.subclass = 0x00;
    e1000_pci.bar[0] = 0xFEBC0000;
    pci_register_device(&e1000_pci);

    assert(e1000_init() == 0);
    assert(e1000_is_present() == 1);

    const e1000_device_t *edev = e1000_get_device();
    assert(edev != NULL);
    assert(edev->mac[0] != 0 || edev->mac[1] != 0 || edev->mac[2] != 0);

    netdev_t *net = netdev_find_by_name("eth0");
    assert(net != NULL);
    assert(net->mtu == 1500);

    /* Test Transmit Packet */
    uint8_t dummy_frame[64];
    memset(dummy_frame, 0xAB, sizeof(dummy_frame));
    int tx_bytes = e1000_transmit(net, dummy_frame, sizeof(dummy_frame));
    assert(tx_bytes == sizeof(dummy_frame));
    assert(net->tx_packets == 1);
    assert(net->tx_bytes == sizeof(dummy_frame));

    /* Test Receive with empty ring */
    uint8_t rx_buf[128];
    int rx_bytes = e1000_receive(net, rx_buf, sizeof(rx_buf));
    assert(rx_bytes == 0); // No hardware frame received

    /* Test ARP Request handling */
    uint8_t arp_req[sizeof(eth_header_t) + sizeof(arp_packet_t)];
    eth_header_t *a_eth = (eth_header_t *)arp_req;
    arp_packet_t *a_arp = (arp_packet_t *)(arp_req + sizeof(eth_header_t));
    memset(a_eth->dest_mac, 0xFF, ETH_ALEN);
    memset(a_eth->src_mac, 0x11, ETH_ALEN);
    a_eth->ethertype = net_htons(ETHERTYPE_ARP);
    a_arp->hw_type = net_htons(1);
    a_arp->proto_type = net_htons(ETHERTYPE_IPV4);
    a_arp->hw_len = ETH_ALEN;
    a_arp->proto_len = 4;
    a_arp->opcode = net_htons(ARP_OP_REQUEST);
    memset(a_arp->sender_mac, 0x11, ETH_ALEN);
    a_arp->sender_ip[0] = 10; a_arp->sender_ip[1] = 0; a_arp->sender_ip[2] = 2; a_arp->sender_ip[3] = 2;
    memset(a_arp->target_mac, 0x00, ETH_ALEN);
    memcpy(a_arp->target_ip, net->ip_addr, 4);

    uint32_t prev_tx = net->tx_packets;
    int arp_res = netdev_process_rx_packet(net, arp_req, sizeof(arp_req));
    assert(arp_res == 1); // Handled ARP
    assert(net->tx_packets == prev_tx + 1); // ARP reply transmitted

    /* Test ICMP Ping Request handling */
    uint8_t icmp_req[sizeof(eth_header_t) + sizeof(ipv4_header_t) + sizeof(icmp_header_t) + 32];
    eth_header_t *i_eth = (eth_header_t *)icmp_req;
    ipv4_header_t *i_ip = (ipv4_header_t *)(icmp_req + sizeof(eth_header_t));
    icmp_header_t *i_icmp = (icmp_header_t *)(icmp_req + sizeof(eth_header_t) + sizeof(ipv4_header_t));
    uint8_t *i_data = icmp_req + sizeof(eth_header_t) + sizeof(ipv4_header_t) + sizeof(icmp_header_t);

    memcpy(i_eth->dest_mac, net->mac_addr, ETH_ALEN);
    memset(i_eth->src_mac, 0x22, ETH_ALEN);
    i_eth->ethertype = net_htons(ETHERTYPE_IPV4);

    i_ip->version_ihl = 0x45;
    i_ip->total_length = net_htons(sizeof(ipv4_header_t) + sizeof(icmp_header_t) + 32);
    i_ip->protocol = IPV4_PROTO_ICMP;
    memcpy(i_ip->dst_ip, net->ip_addr, 4);
    i_ip->src_ip[0] = 10; i_ip->src_ip[1] = 0; i_ip->src_ip[2] = 2; i_ip->src_ip[3] = 2;

    i_icmp->type = ICMP_TYPE_ECHO_REQUEST;
    i_icmp->code = 0;
    i_icmp->checksum = 0;
    memset(i_data, 0x55, 32);
    i_icmp->checksum = net_checksum(i_icmp, sizeof(icmp_header_t) + 32);

    prev_tx = net->tx_packets;
    int icmp_res = netdev_process_rx_packet(net, icmp_req, sizeof(icmp_req));
    assert(icmp_res == 2); // Handled Ping
    assert(net->tx_packets == prev_tx + 1); // Echo reply transmitted

    printf("PASSED\n");
}

static void test_audio_controller_inventory(void) {
    printf("[TEST  4] Running test_audio_controller_inventory          ... ");

    /* Register mock Intel HDA PCI device */
    pci_device_t hda_pci;
    memset(&hda_pci, 0, sizeof(hda_pci));
    hda_pci.bus = 0; hda_pci.dev = 4; hda_pci.func = 0;
    hda_pci.vendor_id = 0x8086; hda_pci.device_id = 0x2668;
    hda_pci.class_code = 0x04; hda_pci.subclass = 0x03;
    hda_pci.bar[0] = 0xFEB80000;
    pci_register_device(&hda_pci);

    assert(audio_init() == 0);
    assert(audio_get_device_count() == 1);

    const audio_device_t *adev = audio_get_device(0);
    assert(adev != NULL);
    assert(adev->type == AUDIO_TYPE_HDA);
    assert(adev->vendor_id == 0x8086);
    assert(adev->device_id == 0x2668);
    assert(adev->num_oss > 0);
    assert(adev->num_iss > 0);

    assert(purple_detect_find_by_name("Intel HDA Codec Controller") != NULL);

    printf("PASSED\n");
}

static void test_apdrv_loading_generated_packages(void) {
    printf("[TEST  5] Running test_apdrv_loading_generated_packages    ... ");
    ksym_init();
    apdrv_subsystem_init();

    const char *driver_paths[] = {
        "build/drivers_apdrv/e1000.apdrv",
        "build/drivers_apdrv/xhci.apdrv",
        "build/drivers_apdrv/hdaudio.apdrv"
    };

    for (int i = 0; i < 3; i++) {
        FILE *f = fopen(driver_paths[i], "rb");
        assert(f != NULL);

        fseek(f, 0, SEEK_END);
        long sz = ftell(f);
        fseek(f, 0, SEEK_SET);
        assert(sz > 240);

        uint8_t *buf = (uint8_t *)malloc(sz);
        assert(buf != NULL);
        assert(fread(buf, 1, sz, f) == (size_t)sz);
        fclose(f);

        apdrv_module_t *mod = NULL;
        int rc = apdrv_load(buf, (uint32_t)sz, &mod);
        assert(rc == 0);
        assert(mod != NULL);
        assert(mod->state == APDRV_STATE_RUNNING);
        assert(mod->wx_enforced == 1);

        /* Verify hardware binding for e1000 and intel_hda (matching PCI devices registered earlier) */
        if (strcmp(mod->hdr.name, "e1000") == 0 || strcmp(mod->hdr.name, "intel_hda") == 0) {
            assert(mod->bound_devices >= 1);
        }

        /* Unload cleanly */
        assert(apdrv_unload(mod->hdr.name) == 0);
        assert(apdrv_find(mod->hdr.name) == NULL);
        free(buf);
    }

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Native Hardware Drivers & APDRV Unit Tests  \n");
    printf("=====================================================\n");

    test_rtc_read_and_formatting();
    test_usb_controller_inventory();
    test_e1000_ethernet_operations();
    test_audio_controller_inventory();
    test_apdrv_loading_generated_packages();

    printf("=====================================================\n");
    printf(" Hardware Driver Tests: 5/5 passed                   \n");
    printf("=====================================================\n");
    return 0;
}
