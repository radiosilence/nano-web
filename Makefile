PKGNAME=nano-web
PKGVERSION=0.0.1
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
PKGDIR=release/$(PKGRELEASE)

nano-clean:
	rm -rf $(PKGDIR)

nano-build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

nano-tar:
	tar czf $(PKGDIR).tar.gz $(PKGDIR)

nano-create:
	mkdir -p $(PKGDIR)/sysroot
	cp README.md $(PKGDIR)

nano-manifest:
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest

nano-add-package:
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

nano-push: nano-clean nano-create nano-manifest nano-build nano-add-package
	ops pkg push $(PKGRELEASE)

nano-bundle: nano-clean nano-create nano-manifest nano-build nano-tar 
	@echo "Release created: $(PKGDIR).tar.gz"
