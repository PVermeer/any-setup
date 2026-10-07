# Create an option to build locally without fetchting own repo
# for sourcing and patching
%bcond local 0

# Source repo
%global author pvermeer
%global source any-setup
%global sourcerepo https://github.com/PVermeer/any-setup
%global tag v0.0.0

Name: any-setup
Version: 0.0.0
Release: 0%{?dist}
License: GPL-3.0 license
Summary: Setup anything with actions
Url: %{sourcerepo}

BuildRequires: systemd-rpm-macros
BuildRequires: git
BuildRequires: rustup
BuildRequires: gcc
BuildRequires: pkgconf-pkg-config
BuildRequires: gtk4-devel
BuildRequires: libadwaita-devel
BuildRequires: dbus-devel

%description
A setup application that can be configured with yaml files (pages) to setup anything for the current user.

%define fqdn org.pvermeer.AnySetup

%define workdir %{_builddir}/%{name}
%define sourcedir %{workdir}/%{source}

%prep
# To apply working changes handle sources / patches locally
# COPR should clone the commited changes
%if %{with local}
  # Get sources - local build
  mkdir -p %{sourcedir}
  cp -r %{_topdir}/SOURCES/* %{sourcedir}
%else
  # Get sources - COPR build
  git clone %{sourcerepo} %{sourcedir}
  cd %{sourcedir}
  git reset --hard %{tag}
  cd %{workdir}
%endif

# Do src stuff
cd %{sourcedir}
rm -rf .git
cd %{workdir}

%build
cd %{sourcedir}
rustup-init -y
source "$HOME/.cargo/env"
cargo build --release

%check

%install
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_datadir}/icons/hicolor/512x512/apps
mkdir -p %{buildroot}%{_datadir}/metainfo
mkdir -p %{buildroot}%{_datadir}/polkit-1/actions

install -Dm755 %{sourcedir}/target/release/any-setup %{buildroot}%{_bindir}
install -Dm644 %{sourcedir}/assets/desktop/%{fqdn}.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/
install -Dm644 %{sourcedir}/assets/desktop/%{fqdn}.metainfo.xml %{buildroot}%{_datadir}/metainfo/
install -Dm644 %{sourcedir}/assets/desktop/%{fqdn}.policy %{buildroot}%{_datadir}/polkit-1/actions/

%files
%{_bindir}/any-setup
%{_datadir}/icons/hicolor/512x512/apps/%{fqdn}.png
%{_datadir}/metainfo/%{fqdn}.metainfo.xml
%{_datadir}/polkit-1/actions/%{fqdn}.policy
