PKGNAME=nano-web
PKGVERSION=0.0.1
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
PKGDIR=release/$(PKGRELEASE)

nano-clean:
	rm -rf $(PKGDIR)

nano-build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

nano-create: nano-clean
	mkdir -p $(PKGDIR)/sysroot
	mkdir -p $(PKGDIR)/sysroot/public
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest
	cp README.md $(PKGDIR)

nano-add-package: nano-create nano-build
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

nano-bundle: nano-create nano-build
	tar czf $(PKGDIR).tar.gz $(PKGDIR)
	@echo "Release created: $(PKGDIR).tar.gz"

nano-push: nano-add-package
	ops pkg push $(PKGRELEASE)

nano-load: nano-add-package
	ops pkg load -l $(PKGRELEASE) -p 80
