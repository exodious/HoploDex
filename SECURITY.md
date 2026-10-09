# Security policy

## Reporting a vulnerability

Please report security vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/exodious/HoploDex/security/advisories/new)
(the repository's **Security** tab, then **Report a vulnerability**). Don't
open a public issue or pull request for one.

A useful report says:

- the version or commit you found it in, and the operating system;
- what an attacker can do, and what they need first (a crafted file to import
  or attach, access to the computer, a compromised web view, and so on);
- how to reproduce it, with the smallest file or steps that show it.

**Never send real collection data.** HoploDex holds sensitive records, so
don't attach a real `.hoplodex` database, backup, document or photo, or a
passphrase. Make a new database with made-up records to reproduce the
problem instead.

## What happens next

The report stays private while it's looked into and fixed. Once a release
with the fix is available, the vulnerability is published as a GitHub
security advisory naming the affected and fixed versions, crediting you if
you'd like that. HoploDex is maintained by one person, so there's no fixed
response time, but every report is read.

## Supported versions

HoploDex hasn't had its first release yet. Once it has, security fixes go into
the latest release.

## Scope

HoploDex is a local-only desktop app: it runs no server and sends collection
data nowhere. Reports about the app itself, its file formats (the encrypted
database, backups, spreadsheet import and export) and the documents it opens
or previews are all in scope. A vulnerability in a dependency belongs with its
own project, but a report is welcome here too if HoploDex's use of it is
exploitable.
