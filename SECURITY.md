# Security policy

## Reporting a vulnerability

Please report security vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/exodious/HoploDex/security/advisories/new)
(the repository's **Security** tab, then **Report a vulnerability**). Don't
open a public issue or pull request for one.

A useful report says:

- the version or commit you found it in, and the operating system;
- what an attacker can do, and what they need first (a crafted file to
  attach, a compromised web view, another program running as the same user,
  and so on);
- how to reproduce it, with the smallest file or steps that show it.

**Never send real collection data.** HoploDex holds sensitive records, so
don't attach a real `.hoplodex` database, backup, document or photo, or a
passphrase. Make a new database with made-up records to reproduce the
problem instead.

## What happens next

The report stays private while it's looked into. If it affects a supported
version (below), it stays private while it's fixed, and once a release with
the fix is available the vulnerability is published as a GitHub security
advisory naming the affected and fixed versions, crediting you if you'd like
that. Otherwise it's fixed through an ordinary public pull request. HoploDex
is maintained by one person, so there's no fixed response time, but every
report is read.

## Supported versions

No version is supported yet. HoploDex 0.1.0 is released, and security fixes
go into the next release through ordinary public pull requests. A version
listed here as supported gets its vulnerabilities fixed privately and
published as security advisories.

## Scope

HoploDex is a local-only desktop app: it runs no server and sends collection
data nowhere. Reports about the app itself, the spreadsheets it exports, and
the documents and photos it attaches, opens or previews (spreadsheets
attached as documents included) are all in scope. A spreadsheet the user
imports is their own data, so an attack that needs one built to attack the
import is out of scope; one the import mishandles (an error that isn't clear,
a change to the collection after a failed import) is a bug to report as an
ordinary issue. A `.hoplodex` database and its backups are private to the
user and never shared, so an attack that needs a database or backup someone
else built or altered is out of scope; a fault in how the app handles a damaged
one, or one from an older version or another of the user's computers, is a bug to
report as an ordinary issue too. An attack that needs a helper process
hijacked from outside HoploDex (an already compromised computer) is out of
scope; one where a hostile attached file takes over a helper is in scope.
Another program running as the same user is in scope only where HoploDex
claims to resist one. A vulnerability in a dependency belongs with its
own project, but a report is welcome here too if HoploDex's use of it is
exploitable.
