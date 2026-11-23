The lists cover the lockfiles exactly, and carry the generators' inline, script and shell entries
in the shapes they write (an esbuild binary's tool cache copy and its shell entry included).
`sources.test.ts` must pass on it (011 US3-2), and its tests for code scanning alert
build-ci/cwe-345 change one entry of a copy of it at a time.
