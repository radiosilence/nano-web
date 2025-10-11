#include <bpf/bpf_endian.h>
#include <bpf/bpf_helpers.h>
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/in.h>
#include <linux/ip.h>
#include <linux/tcp.h>

// Response map: key = (path_hash, encoding), value = pre-baked HTTP response
struct response_key {
  __u32 path_hash;
  __u8 encoding;
} __attribute__((packed));

struct response_value {
  __u32 body_len;
  __u8 content_type[64];
  __u8 content_encoding[16];
  __u8 etag[64];
  __u8 cache_control[64];
  __u8 body[4096]; // 4KB max for now
} __attribute__((packed));

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 10000);
  __type(key, struct response_key);
  __type(value, struct response_value);
} response_map SEC(".maps");

// Simple djb2 hash for path strings
static __always_inline __u32 hash_path(const char *str, int len) {
  __u32 hash = 5381;
  for (int i = 0; i < len && i < 256; i++) {
    __u8 c = str[i];
    if (c == 0 || c == ' ' || c == '\r' || c == '\n')
      break;
    hash = ((hash << 5) + hash) + c;
  }
  return hash;
}

// Parse HTTP GET request and extract path
static __always_inline int parse_http_request(void *data, void *data_end,
                                              __u32 *path_hash,
                                              __u8 *encoding) {
  // HTTP request format: "GET /path HTTP/1.1\r\n"
  char *http = (char *)data;

  // Check we have at least "GET / HTTP"
  if (http + 12 > (char *)data_end)
    return -1;

  // Verify it's a GET request
  if (http[0] != 'G' || http[1] != 'E' || http[2] != 'T' || http[3] != ' ')
    return -1;

  // Find path start (after "GET ")
  char *path_start = http + 4;
  char *path_end = path_start;

// Find path end (space before HTTP/1.1)
#pragma unroll
  for (int i = 0; i < 256; i++) {
    if (path_end + i >= (char *)data_end)
      break;
    if (path_end[i] == ' ')
      break;
    path_end = path_start + i + 1;
  }

  int path_len = path_end - path_start;
  if (path_len <= 0 || path_len > 256)
    return -1;

  *path_hash = hash_path(path_start, path_len);
  *encoding = 0; // TODO: Parse Accept-Encoding header

  return 0;
}

// Calculate IP checksum
static __always_inline __u16 ip_checksum(struct iphdr *ip) {
  __u32 sum = 0;
  __u16 *data = (__u16 *)ip;

#pragma unroll
  for (int i = 0; i < sizeof(struct iphdr) / 2; i++) {
    if (i != 5) // Skip checksum field
      sum += data[i];
  }

  while (sum >> 16)
    sum = (sum & 0xffff) + (sum >> 16);

  return ~sum;
}

// Calculate TCP checksum (simplified, no options)
static __always_inline __u16 tcp_checksum(struct iphdr *ip, struct tcphdr *tcp,
                                          void *data_end, __u32 payload_len) {
  __u32 sum = 0;

  // Pseudo-header
  sum += (ip->saddr >> 16) + (ip->saddr & 0xffff);
  sum += (ip->daddr >> 16) + (ip->daddr & 0xffff);
  sum += bpf_htons(IPPROTO_TCP);
  sum += bpf_htons(sizeof(struct tcphdr) + payload_len);

  // TCP header
  __u16 *tcp_data = (__u16 *)tcp;
#pragma unroll
  for (int i = 0; i < sizeof(struct tcphdr) / 2; i++) {
    if (i != 8) // Skip checksum field
      sum += tcp_data[i];
  }

  // Payload (HTTP response)
  __u16 *payload = (__u16 *)((void *)tcp + sizeof(struct tcphdr));
  int words = payload_len / 2;
#pragma unroll
  for (int i = 0; i < 256; i++) { // Max iterations for verifier
    if (i >= words)
      break;
    if ((void *)(payload + i + 1) > data_end)
      break;
    sum += payload[i];
  }

  // Handle odd byte
  if (payload_len & 1) {
    __u8 *last = (__u8 *)payload + payload_len - 1;
    if ((void *)(last + 1) <= data_end)
      sum += *last;
  }

  while (sum >> 16)
    sum = (sum & 0xffff) + (sum >> 16);

  return ~sum;
}

// Build complete HTTP response packet in place
static __always_inline int build_response(struct xdp_md *ctx,
                                          struct ethhdr *eth_orig,
                                          struct iphdr *ip_orig,
                                          struct tcphdr *tcp_orig,
                                          struct response_value *resp) {
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;

  // Calculate HTTP response size
  char http_headers[512];
  int header_len = 0;

  // Build HTTP response headers
  // "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 123\r\n\r\n"
  char *status = "HTTP/1.1 200 OK\r\n";
#pragma unroll
  for (int i = 0; i < 17; i++) {
    if (header_len + i >= 512)
      break;
    http_headers[header_len++] = status[i];
  }

  // Content-Type
  char *ct_header = "Content-Type: ";
#pragma unroll
  for (int i = 0; i < 14; i++) {
    if (header_len >= 512)
      break;
    http_headers[header_len++] = ct_header[i];
  }
#pragma unroll
  for (int i = 0; i < 64; i++) {
    if (resp->content_type[i] == 0)
      break;
    if (header_len >= 512)
      break;
    http_headers[header_len++] = resp->content_type[i];
  }
  http_headers[header_len++] = '\r';
  http_headers[header_len++] = '\n';

  // Content-Length
  char *cl_header = "Content-Length: ";
#pragma unroll
  for (int i = 0; i < 16; i++) {
    if (header_len >= 512)
      break;
    http_headers[header_len++] = cl_header[i];
  }

  // Convert body_len to string (simple itoa)
  __u32 len = resp->body_len;
  char len_str[16];
  int len_digits = 0;
  do {
    len_str[len_digits++] = '0' + (len % 10);
    len /= 10;
  } while (len > 0 && len_digits < 16);

  // Reverse digits
  for (int i = len_digits - 1; i >= 0; i--) {
    if (header_len >= 512)
      break;
    http_headers[header_len++] = len_str[i];
  }
  http_headers[header_len++] = '\r';
  http_headers[header_len++] = '\n';

  // End of headers
  http_headers[header_len++] = '\r';
  http_headers[header_len++] = '\n';

  __u32 total_http_len = header_len + resp->body_len;
  __u32 new_total_len = sizeof(struct ethhdr) + sizeof(struct iphdr) +
                        sizeof(struct tcphdr) + total_http_len;

  // Calculate how much space we need
  int current_len = data_end - data;
  int delta = new_total_len - current_len;

  // Grow/shrink packet as needed
  if (delta != 0) {
    if (bpf_xdp_adjust_tail(ctx, delta) != 0) {
      // Failed to adjust packet size
      return XDP_DROP;
    }
    // Update pointers after adjustment
    data = (void *)(long)ctx->data;
    data_end = (void *)(long)ctx->data_end;
    eth_orig = data;
    ip_orig = (void *)(eth_orig + 1);
    tcp_orig = (void *)ip_orig + (ip_orig->ihl * 4);
  }

  // Swap MAC addresses
  __u8 tmp_mac[6];
  __builtin_memcpy(tmp_mac, eth_orig->h_dest, 6);
  __builtin_memcpy(eth_orig->h_dest, eth_orig->h_source, 6);
  __builtin_memcpy(eth_orig->h_source, tmp_mac, 6);

  // Swap IP addresses
  __u32 tmp_ip = ip_orig->saddr;
  ip_orig->saddr = ip_orig->daddr;
  ip_orig->daddr = tmp_ip;

  // Update IP header
  ip_orig->tot_len =
      bpf_htons(sizeof(struct iphdr) + sizeof(struct tcphdr) + total_http_len);
  ip_orig->check = 0;
  ip_orig->check = ip_checksum(ip_orig);

  // Swap TCP ports
  __u16 tmp_port = tcp_orig->source;
  tcp_orig->source = tcp_orig->dest;
  tcp_orig->dest = tmp_port;

  // Update TCP header
  __u32 tmp_seq = tcp_orig->seq;
  tcp_orig->seq = tcp_orig->ack_seq;
  tcp_orig->ack_seq = bpf_htonl(bpf_ntohl(tmp_seq) + 1); // ACK the request
  tcp_orig->psh = 1;
  tcp_orig->ack = 1;

  // Copy HTTP response
  void *http_payload = (void *)tcp_orig + sizeof(struct tcphdr);
  if (http_payload + total_http_len > data_end)
    return XDP_DROP;

  __builtin_memcpy(http_payload, http_headers, header_len);
  __builtin_memcpy(http_payload + header_len, resp->body, resp->body_len);

  // Calculate TCP checksum
  tcp_orig->check = 0;
  tcp_orig->check = tcp_checksum(ip_orig, tcp_orig, data_end, total_http_len);

  // Send packet back out the same interface
  return XDP_TX;
}

SEC("xdp")
int http_server_xdp(struct xdp_md *ctx) {
  void *data_end = (void *)(long)ctx->data_end;
  void *data = (void *)(long)ctx->data;

  // Parse Ethernet header
  struct ethhdr *eth = data;
  if ((void *)(eth + 1) > data_end)
    return XDP_PASS;

  // Only handle IPv4
  if (eth->h_proto != bpf_htons(ETH_P_IP))
    return XDP_PASS;

  // Parse IP header
  struct iphdr *ip = (void *)(eth + 1);
  if ((void *)(ip + 1) > data_end)
    return XDP_PASS;

  // Only handle TCP
  if (ip->protocol != IPPROTO_TCP)
    return XDP_PASS;

  // Parse TCP header
  struct tcphdr *tcp = (void *)ip + (ip->ihl * 4);
  if ((void *)(tcp + 1) > data_end)
    return XDP_PASS;

  // Only handle port 3000
  if (tcp->dest != bpf_htons(3000))
    return XDP_PASS;

  // HTTP payload starts after TCP header
  void *http_data = (void *)tcp + (tcp->doff * 4);
  if (http_data >= data_end)
    return XDP_PASS;

  // Parse HTTP request
  __u32 path_hash = 0;
  __u8 encoding = 0;

  if (parse_http_request(http_data, data_end, &path_hash, &encoding) < 0)
    return XDP_PASS;

  // Lookup response in map
  struct response_key key = {
      .path_hash = path_hash,
      .encoding = encoding,
  };

  struct response_value *resp = bpf_map_lookup_elem(&response_map, &key);
  if (!resp)
    return XDP_PASS; // No response found, pass to normal stack

  // Build and send response
  return build_response(ctx, eth, ip, tcp, resp);
}

char _license[] SEC("license") = "GPL";
