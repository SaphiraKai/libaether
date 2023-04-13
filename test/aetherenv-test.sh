#!/bin/bash

set -euo pipefail
export IFS=$'\n'

root=aetherenv-test/

proot='proot'
binds='-b /var/cache/pacman/pkg:/pkgs'

[ $1 = --clean ] && sudo rm -rf $root/* && shift

all_depends() {
		current=${to_search[0]:-}
		[ "$current" = '' ] && exit
		{ printf '%s\0' "${searched[@]}" | grep -F -x -z -- "$current" 2>&1 >/dev/null; } || {
				echo $current | sed 's/[<>]=.*$//'

				searched+=($current)
				dep_string=$(pacman -Qi $current | grep -oP 'Depends On *: \K.*' | sed 's/  /\n/g')
				for dep in $dep_string; do
						[ $dep != None ] && {
								to_search+=($dep)
						}
				done
		}

		to_search=(${to_search[@]:1})
		all_depends
}

pkg_names=($@)
pkgs=()
for pkg in ${pkg_names_names[@]}; do
    pkgs+=$(find . -maxdepth 1 -name "$pkg-*.pkg.tar.zst")
done

echo 'resolving dependencies...'
searched=()
to_search=(${pkg_names[@]})
depends=($(all_depends | sort -u))

echo -e 'resolving providers...\n'
depend_pkgs=()
for pkg in ${depends[@]}; do
    2>&1 >/dev/null pacman -Qiq $pkg \
    && depend_pkgs+=($pkg) \
    || depend_pkgs+=($(pacman -Ssq $(echo $pkg | sed -e 's/ /\n/g' -e 's/=.*$//') | head -n1))
    
    printf '\033[1A'
    printf '\033[K'
    echo ${depend_pkgs[-1]}
done
sudo pacman -Sw --noconfirm --needed ${depend_pkgs[@]}

pacstrap -cN $root pacman

bash -c "$proot ${binds[@]} -S $root pacman --needed --noconfirm -S ${depend_pkgs[@]}"