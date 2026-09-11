DESTDIR ?=
PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APPDIR ?= $(DATADIR)/applications
ICONDIR ?= $(DATADIR)/icons/hicolor/scalable/apps
SYSTEMDUNITDIR ?= $(PREFIX)/lib/systemd/system
SYSTEMDUSERDIR ?= $(PREFIX)/lib/systemd/user
DBUSCONFDIR ?= $(DATADIR)/dbus-1/system.d
POLKITDIR ?= $(DATADIR)/polkit-1/actions
UDEVDIR ?= $(PREFIX)/lib/udev/rules.d
METAINFODIR ?= $(DATADIR)/metainfo
BASHCOMPDIR ?= $(DATADIR)/bash-completion/completions
ZSHCOMPDIR ?= $(DATADIR)/zsh/site-functions
FISHCOMPDIR ?= $(DATADIR)/fish/vendor_completions.d
INSTALL ?= install
CARGO ?= cargo

all: build

build:
	$(CARGO) build --release --bins --features gui

install:
	$(INSTALL) -d $(DESTDIR)$(BINDIR)
	$(INSTALL) -m 755 target/release/alatus $(DESTDIR)$(BINDIR)/alatus
	$(INSTALL) -m 755 target/release/alatusd $(DESTDIR)$(BINDIR)/alatusd
	$(INSTALL) -m 755 target/release/alatus-session $(DESTDIR)$(BINDIR)/alatus-session
	$(INSTALL) -m 755 target/release/alatus-gui $(DESTDIR)$(BINDIR)/alatus-gui
	$(INSTALL) -d $(DESTDIR)$(APPDIR)
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.desktop $(DESTDIR)$(APPDIR)/io.strixwolf.alatus.desktop
	$(INSTALL) -m 644 packaging/alatus-gui.desktop $(DESTDIR)$(APPDIR)/alatus-gui.desktop
	$(INSTALL) -d $(DESTDIR)$(ICONDIR)
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.svg $(DESTDIR)$(ICONDIR)/io.strixwolf.alatus.svg
	$(INSTALL) -m 644 packaging/alatus-gui.svg $(DESTDIR)$(ICONDIR)/alatus-gui.svg
	for size in 32 48 64 128 256; do \
		$(INSTALL) -d $(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps; \
		$(INSTALL) -m 644 packaging/icons/$${size}x$${size}/apps/io.strixwolf.alatus.png $(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/io.strixwolf.alatus.png; \
		$(INSTALL) -m 644 packaging/icons/$${size}x$${size}/apps/alatus-gui.png $(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/alatus-gui.png; \
	done
	$(INSTALL) -d $(DESTDIR)$(DATADIR)/pixmaps
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.svg $(DESTDIR)$(DATADIR)/pixmaps/io.strixwolf.alatus.svg
	$(INSTALL) -m 644 packaging/alatus-gui.svg $(DESTDIR)$(DATADIR)/pixmaps/alatus-gui.svg
	$(INSTALL) -d $(DESTDIR)$(SYSTEMDUNITDIR)
	$(INSTALL) -m 644 packaging/alatusd.service $(DESTDIR)$(SYSTEMDUNITDIR)/alatusd.service
	$(INSTALL) -d $(DESTDIR)$(SYSTEMDUSERDIR)
	$(INSTALL) -m 644 packaging/alatus-session.service $(DESTDIR)$(SYSTEMDUSERDIR)/alatus-session.service
	$(INSTALL) -d $(DESTDIR)$(DBUSCONFDIR)
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.conf $(DESTDIR)$(DBUSCONFDIR)/io.strixwolf.alatus.Daemon.conf
	$(INSTALL) -d $(DESTDIR)$(POLKITDIR)
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.policy $(DESTDIR)$(POLKITDIR)/io.strixwolf.alatus.policy
	$(INSTALL) -d $(DESTDIR)$(UDEVDIR)
	$(INSTALL) -m 644 packaging/udev/99-alatus-rgb.rules $(DESTDIR)$(UDEVDIR)/99-alatus-rgb.rules
	$(INSTALL) -d $(DESTDIR)$(METAINFODIR)
	$(INSTALL) -m 644 packaging/io.strixwolf.alatus.metainfo.xml $(DESTDIR)$(METAINFODIR)/io.strixwolf.alatus.metainfo.xml
	$(INSTALL) -d $(DESTDIR)$(BASHCOMPDIR)
	$(INSTALL) -m 644 packaging/completions/alatus.bash $(DESTDIR)$(BASHCOMPDIR)/alatus
	$(INSTALL) -d $(DESTDIR)$(ZSHCOMPDIR)
	$(INSTALL) -m 644 packaging/completions/_alatus $(DESTDIR)$(ZSHCOMPDIR)/_alatus
	$(INSTALL) -d $(DESTDIR)$(FISHCOMPDIR)
	$(INSTALL) -m 644 packaging/completions/alatus.fish $(DESTDIR)$(FISHCOMPDIR)/alatus.fish

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/alatus
	rm -f $(DESTDIR)$(BINDIR)/alatusd
	rm -f $(DESTDIR)$(BINDIR)/alatus-session
	rm -f $(DESTDIR)$(BINDIR)/alatus-gui
	rm -f $(DESTDIR)$(APPDIR)/io.strixwolf.alatus.desktop
	rm -f $(DESTDIR)$(APPDIR)/alatus-gui.desktop
	rm -f $(DESTDIR)$(ICONDIR)/io.strixwolf.alatus.svg
	rm -f $(DESTDIR)$(ICONDIR)/alatus-gui.svg
	for size in 32 48 64 128 256; do \
		rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/io.strixwolf.alatus.png; \
		rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/alatus-gui.png; \
	done
	rm -f $(DESTDIR)$(DATADIR)/pixmaps/io.strixwolf.alatus.svg
	rm -f $(DESTDIR)$(DATADIR)/pixmaps/alatus-gui.svg
	rm -f $(DESTDIR)$(SYSTEMDUNITDIR)/alatusd.service
	rm -f $(DESTDIR)$(SYSTEMDUSERDIR)/alatus-session.service
	rm -f $(DESTDIR)$(DBUSCONFDIR)/io.strixwolf.alatus.Daemon.conf
	rm -f $(DESTDIR)$(POLKITDIR)/io.strixwolf.alatus.policy
	rm -f $(DESTDIR)$(UDEVDIR)/99-alatus-rgb.rules
	rm -f $(DESTDIR)$(METAINFODIR)/io.strixwolf.alatus.metainfo.xml
	rm -f $(DESTDIR)$(BASHCOMPDIR)/alatus
	rm -f $(DESTDIR)$(ZSHCOMPDIR)/_alatus
	rm -f $(DESTDIR)$(FISHCOMPDIR)/alatus.fish

clean:
	$(CARGO) clean

.PHONY: all build install uninstall clean
