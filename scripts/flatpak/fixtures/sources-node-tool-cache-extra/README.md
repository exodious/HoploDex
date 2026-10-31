A tool-cache archive of a package `package-lock.json` has (left-pad 1.3.0, the same
url and hash as its `file` source) is accepted, as the real list's esbuild copies are.
An archive put there for extra-pkg, which the lockfile does not have, is not:
`checkNodeSources` must fail naming its address.
