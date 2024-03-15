PKGNAME=nano-web
PKGVERSION=0.0.1
PKGDIR=build/$(PKGNAME)_$(PKGVERSION)

nano-build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

nano-tar:
	tar czf $(PKGDIR).tar.gz $(PKGDIR)

nano-create:
	mkdir -p $(PKGDIR)/sysroot
	cp README.md $(PKGDIR)

nano-release: nano-create nano-manifest nano-build nano-tar 
	@echo "Release created: $(PKGDIR).tar.gz"

nano-manifest:
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest