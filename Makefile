PKGNAME=nano-web
PKGVERSION=0.0.1
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
RELEASEDIR=./release
PKGDIR=$(RELEASEDIR)/$(PKGRELEASE)

pkg-clean:
	rm -rf $(RELEASEDIR)

pkg-build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

pkg-create: pkg-clean
	mkdir -p $(PKGDIR)/sysroot
	mkdir -p $(PKGDIR)/sysroot/public
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest
	cp README.md $(PKGDIR)

pkg-add: pkg-create pkg-build
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

pkg-bundle: pkg-add
	tar czvf $(PKGDIR).tar.gz $(PKGDIR)
	@echo "Release created: $(PKGDIR).tar.gz"

pkg-push: pkg-add
	ops pkg push $(PKGRELEASE)

pkg-load: pkg-add
	ops pkg load -l $(PKGRELEASE) -p 80
