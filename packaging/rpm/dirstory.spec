%global srcname dirstory
# Rust attributes in debug sources are not interpreter shebangs.
%global __brp_mangle_shebangs_exclude_from ^%{_prefix}/src/debug/.*\.rs$

%if 0%{?git_build}
%global pkg_name %{srcname}-git
%else
%global pkg_name %{srcname}
%endif

Name:           %{pkg_name}
Version:        0.1.0
Release:        %autorelease
Summary:        Navigate backward and forward through directory visits

# Dependency licenses are verified by cargo_license_summary during the build.
License:        GPL-3.0-or-later AND Apache-2.0 AND MIT AND MPL-2.0 AND Zlib
URL:            https://github.com/nikromen/dirstory
Source0:        %{url}/archive/refs/tags/%{srcname}-%{version}.tar.gz

BuildRequires:  rust >= 1.89
BuildRequires:  cargo
BuildRequires:  cargo-rpm-macros >= 26
BuildRequires:  gcc
BuildRequires:  pandoc
BuildRequires:  python3
BuildRequires:  groff-base
BuildRequires:  man-db
BuildRequires:  bash
BuildRequires:  bash-completion
BuildRequires:  dash
BuildRequires:  zsh
BuildRequires:  fish

%if 0%{?git_build}
Conflicts:      %{srcname}
%else
Conflicts:      %{srcname}-git
%endif

%description
Dirstory tracks directory visits and supplies shell functions for backward and
forward navigation, an interactive history picker, and history recovery.
Load the shell integration explicitly with dirstory init; installing this package
does not modify shell configuration or initialize history.
%if 0%{?git_build}

This is a development build from the main branch.
%endif

%prep
%autosetup -n %{srcname}-%{srcname}-%{version}

%build
%{__cargo} fetch --locked
%{__cargo} build --profile rpm --locked
%cargo_license_summary
%{cargo_license} > LICENSE.dependencies
%cargo_vendor_manifest
pandoc --from=markdown-smart --to=man --standalone man/dirstory.1.md -o dirstory.1
pandoc --from=markdown-smart --to=man --standalone man/dirstory.5.md -o dirstory.5

%install
install -Dpm0755 target/rpm/%{srcname} %{buildroot}%{_bindir}/%{srcname}
install -Dpm0644 dirstory.1 %{buildroot}%{_mandir}/man1/dirstory.1
install -Dpm0644 dirstory.5 %{buildroot}%{_mandir}/man5/dirstory.5
install -Dpm0644 completions/dirstory.bash %{buildroot}%{_datadir}/bash-completion/completions/dirstory
install -Dpm0644 completions/_dirstory %{buildroot}%{_datadir}/zsh/site-functions/_dirstory
install -Dpm0644 completions/dirstory.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/dirstory.fish
for name in b f dh; do
    ln -s dirstory %{buildroot}%{_datadir}/bash-completion/completions/$name
    ln -s dirstory.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/$name.fish
done

%check
%{__cargo} test --profile rpm --locked -- --test-threads=1
DIRSTORY_BIN=%{buildroot}%{_bindir}/%{srcname} \
    DIRSTORY_REQUIRE_SHELLS=1 E2E_SHELL= E2E_FILTER= \
    python3 tests/e2e/runtest.py
bash -n %{buildroot}%{_datadir}/bash-completion/completions/dirstory
zsh -n %{buildroot}%{_datadir}/zsh/site-functions/_dirstory
fish -n %{buildroot}%{_datadir}/fish/vendor_completions.d/dirstory.fish
for section in 1 5; do
    groff -man -Tutf8 -z -ww dirstory.$section 2> man-warnings
    cat man-warnings
    test ! -s man-warnings
done
python3 packaging/rpm/check_completions.py %{buildroot}

%files
%license LICENSE LICENSE.dependencies cargo-vendor.txt
%doc README.md
%{_bindir}/%{srcname}
%{_mandir}/man1/dirstory.1*
%{_mandir}/man5/dirstory.5*
%{_datadir}/bash-completion/completions/dirstory
%{_datadir}/bash-completion/completions/b
%{_datadir}/bash-completion/completions/f
%{_datadir}/bash-completion/completions/dh
%{_datadir}/zsh/site-functions/_dirstory
%{_datadir}/fish/vendor_completions.d/dirstory.fish
%{_datadir}/fish/vendor_completions.d/b.fish
%{_datadir}/fish/vendor_completions.d/f.fish
%{_datadir}/fish/vendor_completions.d/dh.fish

%changelog
%autochangelog
