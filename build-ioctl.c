// SPDX-License-Identifier: 0BSD
// Derived from https://git.sr.ht/~nabijaczleweli/voreutils/tree/02bcd701febb555147b67e0fa7fdc1504fe3cca2/item/cmd/wc.cpp#L155-177

#include <inttypes.h>
#include <sys/ioctl.h>
#include <sys/types.h>
#if __linux__
#include <linux/fs.h>
#elif __OpenBSD__
#include <sys/disklabel.h>
#include <sys/dkio.h>
#elif __has_include(<sys/disk.h>)  // NetBSD, FreeBSD
#include <sys/disk.h>
#include <sys/disklabel.h>
#include <sys/types.h>
#elif __has_include(<sys/vtoc.h>)  // illumos
#include <sys/dkio.h>
#include <sys/vtoc.h>
#endif


extern uint64_t http_blkgetsize(int fd);
uint64_t http_blkgetsize(int fd) {
	int ret = -1;

#ifdef BLKGETSIZE64  // Linux
	uint64_t sz;
	ret = ioctl(fd, BLKGETSIZE64, &sz);
#elif defined(DIOCGMEDIASIZE)   // NetBSD disk(9), FreeBSD disk(4)
	off_t sz;
	ret = ioctl(fd, DIOCGMEDIASIZE, &sz);
#elif defined(DIOCGDINFO)       // OpenBSD
	struct disklabel dl;
	ret         = ioctl(fd, DIOCGDINFO, &dl);
	uint64_t sz = DL_GETDSIZE(&dl);
	if(__builtin_mul_overflow(sz, dl.d_secsize, &sz))
		sz = -1;
#elif defined(DKIOCGMEDIAINFO)  // illumos
	struct dk_minfo mi;
	ret         = ioctl(fd, DKIOCGMEDIAINFO, &mi);
	uint64_t sz = mi.dki_capacity;
	if(__builtin_mul_overflow(sz, mi.dki_lbsize, &sz))
		sz = -1;
#endif

	if(ret == -1)
		return -1;
	else
		return sz;
}
