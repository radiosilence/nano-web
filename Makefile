PKGOS=linux
PKGARCH=amd64
PKGNAME=nano-web
PKGVERSION:=$(shell ./scripts/get-version.sh)
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
RELEASEDIR=./release
PKGDIR=$(RELEASEDIR)/$(PKGRELEASE)

pkg-clean:
	rm -rf $(RELEASEDIR)

pkg-build:
	 GOOS=$(PKGOS) GOARCH=$(PKGARCH) go build -o $(PKGDIR)/$(PKGNAME) main.go

pkg-create: pkg-clean
	mkdir -p $(PKGDIR)/sysroot
	mkdir -p $(PKGDIR)/sysroot/public
	PKGVERSION=$(PKGVERSION) PKGNAME=$(PKGNAME) ./scripts/make-manifest.sh > $(PKGDIR)/package.manifest
	cp README.md $(PKGDIR)

pkg-add: pkg-create pkg-build
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

pkg-bundle: pkg-add
	cd $(RELEASEDIR); tar czvf $(PKGRELEASE)-$(PKGARCH).tar.gz $(PKGRELEASE)
	@echo "Release created: $(PKGDIR)-$(PKGARCH).tar.gz"

pkg-push: pkg-add
	ops pkg push $(PKGRELEASE)

pkg-load: pkg-add
	ops pkg load -l $(PKGRELEASE) -p 80
