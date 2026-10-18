// One definition of "shipped" for the audit and the notices (010 research.md
// §5, §6): the npm packages from the lockfile (here) and the crates from
// `cargo tree`.

/**
 * Every package in a parsed package-lock.json except the root, with whether it
 * is development-only. A package the lockfile marks `devOptional` (an optional
 * peer installed for type checking, such as `@types/react`) is development-only
 * too: it is never bundled (010 research.md §6).
 */
export function npmPackages(lock) {
  const packages = [];
  for (const [location, pkg] of Object.entries(lock.packages)) {
    if (location === "") continue;
    packages.push({
      name: pkg.name ?? location.slice(location.lastIndexOf("node_modules/") + 13),
      version: pkg.version,
      location,
      license: pkg.license,
      development: Boolean(pkg.dev || pkg.devOptional),
    });
  }
  return packages;
}

/** The npm packages that ship in the interface: everything not development-only. */
export function npmShipped(lock) {
  return npmPackages(lock).filter((pkg) => !pkg.development);
}
