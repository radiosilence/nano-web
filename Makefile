PKGNAME=nano-web
PKGVERSION=0.0.1
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
PKGDIR=release/$(PKGRELEASE)

pkg-clean:
	rm -rf $(PKGDIR)

pkg-build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

pkg-create: pkg-clean
	mkdir -p $(PKGDIR)/sysroot
	mkdir -p $(PKGDIR)/sysroot/public
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest
	cp README.md $(PKGDIR)

pkg-add-package: pkg-create pkg-build
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

pkg-bundle: pkg-create pkg-build
	tar czf $(PKGDIR).tar.gz $(PKGDIR)
	@echo "Release created: $(PKGDIR).tar.gz"

pkg-push: pkg-add-package
	ops pkg push $(PKGRELEASE)

pkg-load: pkg-add-package
	ops pkg load -l $(PKGRELEASE) -p 80
