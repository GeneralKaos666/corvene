# Sources of the bundled git toolchain. `build.sh` checks every download
# against the SHA-256 here; bump a version and its hash together. A URL may
# be a space-separated list of mirrors, tried in order.
GIT_VERSION=2.56.0
GIT_URL="https://mirrors.edge.kernel.org/pub/software/scm/git/git-$GIT_VERSION.tar.xz"
GIT_SHA256=26c56c296b38c0695b26fa95f475f1d01704d2d38e73465ca30b0b2f5dc789d3

OPENSSL_VERSION=3.5.9
OPENSSL_URL="https://github.com/openssl/openssl/releases/download/openssl-$OPENSSL_VERSION/openssl-$OPENSSL_VERSION.tar.gz"
OPENSSL_SHA256=603f5602e2eef00d77fbd429d34dcd5822bb301757a1bc9cdb24c670f1eb859a

NGHTTP2_VERSION=1.64.0
NGHTTP2_URL="https://github.com/nghttp2/nghttp2/releases/download/v$NGHTTP2_VERSION/nghttp2-$NGHTTP2_VERSION.tar.xz"
NGHTTP2_SHA256=88bb94c9e4fd1c499967f83dece36a78122af7d5fb40da2019c56b9ccc6eb9dd

CURL_VERSION=8.22.0
CURL_URL="https://curl.se/download/curl-$CURL_VERSION.tar.xz"
CURL_SHA256=f7ef3ae8a22e521f289803fe93543eb64c329b58aa73a9e224dfd915a2a5f4f7

LIBICONV_VERSION=1.18
# ftp.gnu.org is often unreachable from CI runners: a mirror first
LIBICONV_URL="https://mirrors.kernel.org/gnu/libiconv/libiconv-$LIBICONV_VERSION.tar.gz https://ftpmirror.gnu.org/gnu/libiconv/libiconv-$LIBICONV_VERSION.tar.gz https://ftp.gnu.org/pub/gnu/libiconv/libiconv-$LIBICONV_VERSION.tar.gz"
LIBICONV_SHA256=3b08f5f4f9b4eb82f151a7040bfd6fe6c6fb922efe4b1659c66ea933276965e8

OPENSSH_VERSION=10.5p1
OPENSSH_URL="https://cdn.openbsd.org/pub/OpenBSD/OpenSSH/portable/openssh-$OPENSSH_VERSION.tar.gz"
OPENSSH_SHA256=d44d28a839ea9daf969cc69150fde59910b2b39361dad81a3bd6cbd19218db11

GIT_LFS_VERSION=3.8.0
GIT_LFS_URL="https://github.com/git-lfs/git-lfs/releases/download/v$GIT_LFS_VERSION/git-lfs-v$GIT_LFS_VERSION.tar.gz"
GIT_LFS_SHA256=4f75492c6832038fa73d39a45316657208bb6caa23b273451cb4ec2358d42ccb
