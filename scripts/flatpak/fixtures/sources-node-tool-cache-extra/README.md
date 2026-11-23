An archive put in the tool cache for extra-pkg, which the lockfile does not have, is not
a copy of a lockfile package: `checkNodeSources` must fail naming its address. (The copies
it accepts, an `@esbuild/` package's in its own folder, are in `sources-ok`.)
