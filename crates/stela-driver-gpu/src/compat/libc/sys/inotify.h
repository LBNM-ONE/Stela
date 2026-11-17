/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef STELA_LIBC_SYS_INOTIFY_H
#define STELA_LIBC_SYS_INOTIFY_H
#include <sys/cdefs.h>
#include <stdint.h>
__BEGIN_DECLS
/* Linux's values. RADV watches a file it is told to read again on change. */
#define IN_CLOEXEC 02000000
#define IN_NONBLOCK 04000
#define IN_MODIFY 0x00000002
#define IN_CLOSE_WRITE 0x00000008
#define IN_DELETE_SELF 0x00000400
#define IN_IGNORED 0x00008000
struct inotify_event {
	int wd;
	uint32_t mask;
	uint32_t cookie;
	uint32_t len;
	char name[];
};
int inotify_init1(int flags);
int inotify_add_watch(int fd, const char *path, uint32_t mask);
int inotify_rm_watch(int fd, int watch);
__END_DECLS
#endif
