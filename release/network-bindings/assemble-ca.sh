#!/bin/sh
set -eu
# Replace the complete store and its package metadata before copying a clean tree
# into scratch. A later rm layer would leave the obsolete store downloadable.
rm -rf /rootfs/etc/ssl/certs /rootfs/usr/share/ca-certificates
rm -f /rootfs/var/lib/dpkg/status.d/ca-certificates /rootfs/var/lib/dpkg/status.d/ca-certificates.md5sums
mkdir -p /rootfs/etc/ssl /rootfs/usr/share /rootfs/var/lib/dpkg/status.d /rootfs/usr/share/doc/ca-certificates
cp -a /etc/ssl/certs /rootfs/etc/ssl/
cp -a /usr/share/ca-certificates /rootfs/usr/share/
cp /etc/ca-certificates.conf /rootfs/etc/ca-certificates.conf
dpkg-query -s ca-certificates > /rootfs/var/lib/dpkg/status.d/ca-certificates
cp /var/lib/dpkg/info/ca-certificates.md5sums /rootfs/var/lib/dpkg/status.d/ca-certificates.md5sums
cp /usr/share/doc/ca-certificates/copyright /rootfs/usr/share/doc/ca-certificates/copyright
test "$(dpkg-query -W -f='${Version}' ca-certificates)" = '20250419+deb13u1'
