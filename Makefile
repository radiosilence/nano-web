PKGNAME=nano-web
PKGVERSION=0.0.1
PKGRELEASE=$(PKGNAME)_$(PKGVERSION)
PKGDIR=release/$(PKGRELEASE)

clean:
	rm -rf $(PKGDIR)

build:
	 GOOS=linux go build -o $(PKGDIR)/$(PKGNAME) main.go

create: clean
	mkdir -p $(PKGDIR)/sysroot
	mkdir -p $(PKGDIR)/sysroot/public
	./scripts/make-manifest.sh > $(PKGDIR)/package.manifest
	cp README.md $(PKGDIR)

add-package: create build
	ops pkg add $(PKGDIR) --name $(PKGRELEASE)

bundle: create build
	tar czf $(PKGDIR).tar.gz $(PKGDIR)
	@echo "Release created: $(PKGDIR).tar.gz"

push: add-package
	ops pkg push $(PKGRELEASE)

load: add-package
	ops pkg load -l $(PKGRELEASE) -p 80
