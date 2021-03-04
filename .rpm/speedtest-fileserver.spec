%define __spec_install_post %{nil}
%define __os_install_post %{_dbpath}/brp-compress
%define debug_package %{nil}

Name: speedtest-fileserver
Summary: Speedtest fileserver
Version: @@VERSION@@
Release: @@RELEASE@@%{?dist}
License: MIT
Group: System Environment/Daemons
Source0: %{name}-%{version}.tar.gz
URL: https://github.com/XS4ALL/speedtest-fileserver-rs

BuildRoot: %{_tmppath}/%{name}-%{version}-%{release}-root
BuildRequires: systemd

%description
%{summary}

%prep
%setup -q

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a * %{buildroot}
mkdir -m 755 -p %{buildroot}/var/log/speedtest-fileserver

%clean
rm -rf %{buildroot}

%post
%systemd_post speedtest-fileserver.service
echo
echo "if this is the first install:"
echo "- edit /etc/speedtest-fileserver.conf"
echo "- systemctl enable speedtest-fileserver"
echo "- systemctl start speedtest-fileserver"
echo

%preun
%systemd_preun speedtest-fileserver.service

%postun
%systemd_postun_with_restart speedtest-fileserver.service

%files
%defattr(-,root,root,-)
%{_sbindir}/*
%{_unitdir}/speedtest-fileserver.service

%dir /var/log/speedtest-fileserver
%attr(0644,root,root) %config(noreplace) /etc/speedtest-fileserver.cfg
%attr(0644,root,root) %config(noreplace) /etc/logrotate.d/speedtest-fileserver.conf
