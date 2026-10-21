A tool-cache archive with left-pad 1.3.0's url but another hash is not a copy of the
lockfile's tarball, so it counts as a second source: `checkNodeSources` must fail,
naming left-pad as having 2 sources.
