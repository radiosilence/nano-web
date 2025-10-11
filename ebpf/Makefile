.PHONY: all clean

BPF_TARGET = http_server
BPF_C = $(BPF_TARGET).c
BPF_OBJ = $(BPF_TARGET).o

CLANG ?= clang
LLC ?= llc
ARCH := $(shell uname -m | sed 's/x86_64/x86/' | sed 's/aarch64/arm64/')

# Kernel headers (adjust path as needed)
KERNEL_HEADERS ?= /usr/src/linux-headers-$(shell uname -r)
LIBBPF_INCLUDE ?= /usr/include

INCLUDES = -I$(KERNEL_HEADERS)/include \
           -I$(KERNEL_HEADERS)/include/uapi \
           -I$(KERNEL_HEADERS)/arch/$(ARCH)/include \
           -I$(KERNEL_HEADERS)/arch/$(ARCH)/include/uapi \
           -I$(LIBBPF_INCLUDE)

CFLAGS = -O2 -g -target bpf -D__TARGET_ARCH_$(ARCH) \
         -Wall -Werror \
         $(INCLUDES)

all: $(BPF_OBJ)

$(BPF_OBJ): $(BPF_C)
	@echo "Compiling eBPF program: $(BPF_C) -> $(BPF_OBJ)"
	$(CLANG) $(CFLAGS) -c $< -o $@
	@echo "✓ eBPF program compiled successfully"

clean:
	rm -f $(BPF_OBJ)

install: $(BPF_OBJ)
	@echo "eBPF program ready at: $(PWD)/$(BPF_OBJ)"
	@echo ""
	@echo "To load manually:"
	@echo "  sudo ip link set dev <interface> xdp obj $(BPF_OBJ)"
	@echo ""
	@echo "To unload:"
	@echo "  sudo ip link set dev <interface> xdp off"

check:
	@which clang > /dev/null || (echo "Error: clang not found. Install: apt-get install clang" && exit 1)
	@which llvm-objdump > /dev/null || (echo "Warning: llvm-objdump not found. Install: apt-get install llvm" && exit 1)
	@test -d $(KERNEL_HEADERS) || (echo "Warning: Kernel headers not found at $(KERNEL_HEADERS)" && exit 1)
	@echo "✓ Build environment ready"

info:
	@echo "eBPF Build Configuration:"
	@echo "  Architecture: $(ARCH)"
	@echo "  Clang: $(shell which clang)"
	@echo "  Kernel headers: $(KERNEL_HEADERS)"
	@echo "  Target: $(BPF_OBJ)"
