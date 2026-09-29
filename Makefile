PREFIX ?= /usr
DESTDIR ?=

.PHONY: all build check test clean install uninstall

all: build

build:
	cargo build --release

check:
	cargo check --workspace

test:
	cargo test --workspace

clean:
	cargo clean

install: build
	# Install Binaries
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 target/release/alatusd $(DESTDIR)$(PREFIX)/bin/alatusd
	install -m 755 target/release/alatus-session $(DESTDIR)$(PREFIX)/bin/alatus-session
	install -m 755 target/release/alatus-cli $(DESTDIR)$(PREFIX)/bin/alatus
	install -m 755 target/release/alatus-gui $(DESTDIR)$(PREFIX)/bin/alatus-gui

	# Install Machine Profiles
	install -d $(DESTDIR)$(PREFIX)/share/alatus/profiles
	install -m 644 data/profiles/*.toml $(DESTDIR)$(PREFIX)/share/alatus/profiles/

	# Install D-Bus Configuration
	install -d $(DESTDIR)$(PREFIX)/share/dbus-1/system.d
	install -m 644 data/dbus-1/system.d/org.alatus.Daemon.conf $(DESTDIR)$(PREFIX)/share/dbus-1/system.d/

	# Install Polkit Policy
	install -d $(DESTDIR)$(PREFIX)/share/polkit-1/actions
	install -m 644 data/polkit-1/actions/org.alatus.policy $(DESTDIR)$(PREFIX)/share/polkit-1/actions/

	# Install Systemd Service Units
	install -d $(DESTDIR)$(PREFIX)/lib/systemd/system
	install -m 644 data/systemd/system/alatusd.service $(DESTDIR)$(PREFIX)/lib/systemd/system/

	install -d $(DESTDIR)$(PREFIX)/lib/systemd/user
	install -m 644 data/systemd/user/alatus-session.service $(DESTDIR)$(PREFIX)/lib/systemd/user/

	# Install Desktop Entry
	install -d $(DESTDIR)$(PREFIX)/share/applications
	install -m 644 data/applications/org.alatus.gui.desktop $(DESTDIR)$(PREFIX)/share/applications/

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/alatusd
	rm -f $(DESTDIR)$(PREFIX)/bin/alatus-session
	rm -f $(DESTDIR)$(PREFIX)/bin/alatus
	rm -f $(DESTDIR)$(PREFIX)/bin/alatus-gui
	rm -rf $(DESTDIR)$(PREFIX)/share/alatus
	rm -f $(DESTDIR)$(PREFIX)/share/dbus-1/system.d/org.alatus.Daemon.conf
	rm -f $(DESTDIR)$(PREFIX)/share/polkit-1/actions/org.alatus.policy
	rm -f $(DESTDIR)$(PREFIX)/lib/systemd/system/alatusd.service
	rm -f $(DESTDIR)$(PREFIX)/lib/systemd/user/alatus-session.service
	rm -f $(DESTDIR)$(PREFIX)/share/applications/org.alatus.gui.desktop
