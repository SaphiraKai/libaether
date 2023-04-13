#!/bin/bash

set -euo pipefail
export IFS=$'\n'

search() {
		current=${to_search[0]:-}
		[ "$current" = '' ] && exit
		{ printf '%s\0' "${searched[@]}" | grep -F -x -z -- $current 2>&1 >/dev/null; } || {
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
		search
}

pkg_names=($@)

to_search=(${pkg_names[@]})
searched=()
search
