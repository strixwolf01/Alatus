%global debug_package %{nil}
%define _build_id_links none

Name:           alatus
Version:        0.1.0
Release:        1%{?dist}
Summary:        Linux hardware control suite for ASUS laptops
License:        GPL-3.0-or-later
URL:            https://github.com/strixwolf01/Alatus

Requires:       systemd
Requires:       dbus
Requires:       polkit
Requires:       hicolor-icon-theme

Obsoletes:      alatus < %{version}-%{release}
Provides:       alatus = %{version}-%{release}

%description
Alatus is a clean-slate, modular Linux hardware control suite for ASUS laptops.
It provides fine-grained control over battery charging thresholds, thermal fan
profiles (Quiet, Balanced, Performance, Full Speed), keyboard RGB lighting,
and ASUS WMI hotkey event orchestration.

%prep

%build

%install
rm -rf %{buildroot}
make -C %{_sourcedir} install DESTDIR=%{buildroot} PREFIX=/usr

%post
if [ -d /run/systemd/system ]; then
    systemctl daemon-reload >/dev/null 2>&1 || :
    systemctl reload dbus.service >/dev/null 2>&1 || :
    systemctl try-restart alatusd.service >/dev/null 2>&1 || :
fi
systemctl enable --now alatusd.service >/dev/null 2>&1 || :
systemctl --global enable alatus-session.service >/dev/null 2>&1 || :

%preun
if [ $1 -eq 0 ]; then
    systemctl disable --now alatusd.service >/dev/null 2>&1 || :
    systemctl --global disable alatus-session.service >/dev/null 2>&1 || :
fi

%postun
if [ $1 -eq 0 ]; then
    if [ -d /run/systemd/system ]; then
        systemctl daemon-reload >/dev/null 2>&1 || :
    fi
fi

%files
%{_bindir}/alatus
%{_bindir}/alatusd
%{_bindir}/alatus-session
%{_bindir}/alatus-gui
%{_datadir}/alatus/
%{_datadir}/dbus-1/system.d/org.alatus.Daemon.conf
%{_datadir}/polkit-1/actions/org.alatus.policy
%{_prefix}/lib/systemd/system/alatusd.service
%{_prefix}/lib/systemd/user/alatus-session.service
%{_datadir}/applications/org.alatus.gui.desktop
%{_datadir}/icons/hicolor/scalable/apps/alatus-gui.svg

%changelog
* Tue Sep 29 2026 StrixWolf <strixwolf@example.com> - 0.1.0-1
- Initial release of clean-slate Rust rewrite (0.1.0)
