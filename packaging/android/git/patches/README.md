Patches applied to the sources `../build.sh` builds.

`git-*.patch` come from Termux's git package
(https://github.com/termux/termux-packages/tree/master/packages/git,
Apache-2.0 build recipes):

- `git-run-command.c.patch`: bionic has no `pthread_setcancelstate`.
- `git-disable-fdsan.patch`: git closes file descriptors in ways Android's
  fdsan aborts on.
- `git-config.c.patch`: `.git/config` cannot be chmod-ed on shared storage;
  warn instead of failing.

`openssh-openbsd-compat_explicit_bzero.c.patch` and
`openssh-hostfile.c.patch` come from Termux's openssh package: bionic
declares no `bzero` there, and an app may not create hard links (the
`known_hosts` backup).

`openssh-home.patch` is Corvane's: ssh takes the home directory from
`$HOME`.
