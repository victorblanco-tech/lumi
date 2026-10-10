/* SPDX-License-Identifier: EPL-2.0
 * Narrow, unprivileged ONC RPC v2 / MOUNT v1 / NFS v2 marker server.
 * No exports, directory enumeration, writes, registration or shell commands.
 * A single wildcard socket preserves destination identity using IP_PKTINFO.
 * stdin EOF terminates this child; it is never installed as a system service.
 */
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

typedef struct {
    struct in_addr address;
    int root;
    bool present;
    unsigned fault; /* 0 normal, 1 timeout, 2 missing, 3 malformed */
    uint8_t root_handle[32], file_handle[32], bytes[4096];
    size_t size;
    unsigned revision;
} Slot;
static Slot slots[2];
static int slot_count;
static volatile sig_atomic_t stop;
static void stopping(int sig) { (void)sig; stop = 1; }
static uint32_t get(const uint8_t *p) { uint32_t v; memcpy(&v,p,4); return ntohl(v); }
static void put(uint8_t **p, uint32_t v) { v=htonl(v); memcpy(*p,&v,4); *p+=4; }
static bool skip_auth(const uint8_t *b, size_t n, size_t *off) {
    if (*off+8>n) return false;
    uint32_t size=get(b+*off+4);
    if (size>400 || *off+8+((size+3)&~3U)>n) return false;
    *off+=8+((size+3)&~3U); return true;
}
static bool text_is(const uint8_t *b, size_t n, const char *text) {
    size_t len=strlen(text)*2;
    if (n<4 || get(b)!=len || n!=4+((len+3)&~3U)) return false;
    for (size_t i=0;i<len/2;i++) if(b[4+2*i]!=(uint8_t)text[i] || b[5+2*i]) return false;
    return true;
}
static void new_mount(Slot *s) {
    arc4random_buf(s->root_handle,32); arc4random_buf(s->file_handle,32);
    s->size=0; s->revision++;
}
/* Return NFS status, never serve more than 4 KiB or follow a marker symlink. */
static unsigned refresh(Slot *s) {
    if(!s->present) return 70; /* STALE */
    if(s->fault==2) return 2;
    uint8_t bytes[4097];
    int fd=openat(s->root,".lumi-media.json",O_RDONLY|O_NOFOLLOW|O_NONBLOCK);
    if(fd<0) return errno==ENOENT?2:13;
    struct stat before,after;
    if(fstat(fd,&before) || !S_ISREG(before.st_mode) || before.st_size<1 || before.st_size>4096) {
        close(fd); return 13;
    }
    ssize_t count=read(fd,bytes,sizeof(bytes));
    int bad=fstat(fd,&after); close(fd);
    if(bad || count!=before.st_size || after.st_size!=before.st_size ||
       after.st_mtimespec.tv_sec!=before.st_mtimespec.tv_sec ||
       after.st_mtimespec.tv_nsec!=before.st_mtimespec.tv_nsec) return 70;
    if(s->fault==3) { memcpy(bytes,"{invalid",8); count=8; }
    if(s->size!=(size_t)count || memcmp(s->bytes,bytes,(size_t)count)) {
        memcpy(s->bytes,bytes,(size_t)count); s->size=(size_t)count;
        arc4random_buf(s->file_handle,32); s->revision++;
    }
    return 0;
}
static void attrs(uint8_t **p, Slot *s) {
    uint32_t a[17]={1,0100444,1,0,0,(uint32_t)s->size,4096,0,1,1,2,
                   s->revision,0,s->revision,0,s->revision,0};
    for(int i=0;i<17;i++) put(p,a[i]);
}
static size_t response(Slot *s,const uint8_t *b,size_t n,uint8_t *out,unsigned port) {
    if(n<40 || get(b+4)!=0 || get(b+8)!=2) return 0;
    size_t off=24;
    if(!skip_auth(b,n,&off) || !skip_auth(b,n,&off)) return 0;
    unsigned program=get(b+12), version=get(b+16), proc=get(b+20);
    uint8_t *p=out;
    put(&p,get(b)); put(&p,1); put(&p,0); put(&p,0); put(&p,0);
    uint8_t *accepted=p; put(&p,0);
    if(!((program==100000 && version==2)||(program==100005 && version==1)||(program==100003 && version==2))) {
        uint8_t *q=accepted; put(&q,1); return (size_t)(p-out);
    }
    if(proc==0) return (size_t)(p-out);
    if(program==100000 && proc==3 && n-off==16) {
        unsigned target=get(b+off), v=get(b+off+4), proto=get(b+off+8);
        put(&p,proto==17 && ((target==100005 && v==1)||(target==100003 && v==2))?port:0);
    } else if(program==100005 && proc==1) {
        bool path=text_is(b+off,n-off,"/C/");
        put(&p,path && s->present?0:2);
        if(path && s->present) { memcpy(p,s->root_handle,32); p+=32; }
    } else if(program==100003 && proc==4 && n-off>=36) {
        unsigned status=!s->present || memcmp(b+off,s->root_handle,32)?70:
            !text_is(b+off+32,n-off-32,".lumi-media.json")?2:refresh(s);
        put(&p,status);
        if(!status) { memcpy(p,s->file_handle,32); p+=32; attrs(&p,s); }
    } else if(program==100003 && proc==6 && n-off==44) {
        unsigned status=refresh(s);
        unsigned offset=get(b+off+32), count=get(b+off+36);
        if(!status && memcmp(b+off,s->file_handle,32)) status=70;
        if(!status && (count<1 || count>1024 || offset>s->size || count>s->size-offset)) status=13;
        put(&p,status);
        if(!status) { attrs(&p,s); put(&p,count); memcpy(p,s->bytes+offset,count); p+=count;
            while((p-out)%4) *p++=0; }
    } else { uint8_t *q=accepted; put(&q,3); } /* PROC_UNAVAIL, including every write */
    return (size_t)(p-out);
}
int main(int argc,char **argv) {
    if(argc!=4 && argc!=6) { fprintf(stderr,"Usage: media-rpc PORT IP ROOT [IP ROOT]\n"); return 2; }
    char *end; long port=strtol(argv[1],&end,10);
    if(*end || port<1 || port>65535 || getuid()==0) return 2;
    slot_count=(argc-2)/2;
    for(int i=0;i<slot_count;i++) {
        if(inet_pton(AF_INET,argv[2+2*i],&slots[i].address)!=1) return 2;
        slots[i].root=open(argv[3+2*i],O_RDONLY|O_DIRECTORY|O_NOFOLLOW);
        if(slots[i].root<0) { perror("USB root"); return 2; }
        slots[i].present=true; new_mount(&slots[i]);
    }
    if(slot_count==2 && slots[0].address.s_addr==slots[1].address.s_addr) return 2;
    int sock=socket(AF_INET,SOCK_DGRAM,0), yes=1;
    struct sockaddr_in local={.sin_len=sizeof(local),.sin_family=AF_INET,.sin_port=htons((uint16_t)port)};
    if(sock<0 || setsockopt(sock,IPPROTO_IP,IP_PKTINFO,&yes,sizeof(yes)) ||
       bind(sock,(struct sockaddr*)&local,sizeof(local))) { perror("RPC socket"); return 3; }
    signal(SIGTERM,stopping); signal(SIGINT,stopping); signal(SIGPIPE,SIG_IGN);
    puts("READY"); fflush(stdout);
    struct pollfd fds[2]={{.fd=STDIN_FILENO,.events=POLLIN},{.fd=sock,.events=POLLIN}};
    char line[64]; size_t used=0; time_t second=0; unsigned requests=0;
    while(!stop) {
        if(poll(fds,2,1000)<0) { if(errno==EINTR) continue; break; }
        if(fds[0].revents&(POLLIN|POLLHUP|POLLERR)) {
            char ch; ssize_t r=read(STDIN_FILENO,&ch,1); if(r<=0) break;
            if(ch=='\n') { line[used]=0; unsigned index,present,fault;
                if(sscanf(line,"%u %u %u",&index,&present,&fault)==3 && index<(unsigned)slot_count && present<=1 && fault<=3) {
                    Slot *s=&slots[index]; if(s->present!=(bool)present) new_mount(s);
                    s->present=present; s->fault=fault;
                } used=0;
            } else if(used<sizeof(line)-1) line[used++]=ch; else break;
        }
        if(!(fds[1].revents&POLLIN)) continue;
        uint8_t request[2048],reply[2048]; char ancillary[512];
        struct sockaddr_in peer; struct iovec io={request,sizeof(request)};
        struct msghdr msg={.msg_name=&peer,.msg_namelen=sizeof(peer),.msg_iov=&io,.msg_iovlen=1,
            .msg_control=ancillary,.msg_controllen=sizeof(ancillary)};
        ssize_t n=recvmsg(sock,&msg,0); if(n<0 || msg.msg_flags&(MSG_TRUNC|MSG_CTRUNC)) continue;
        struct in_pktinfo info={0}; bool found=false;
        for(struct cmsghdr *c=CMSG_FIRSTHDR(&msg);c;c=CMSG_NXTHDR(&msg,c))
            if(c->cmsg_level==IPPROTO_IP && c->cmsg_type==IP_PKTINFO && c->cmsg_len>=CMSG_LEN(sizeof(info))) {
                memcpy(&info,CMSG_DATA(c),sizeof(info)); found=true;
            }
        if(!found) continue;
        if(getenv("LUMI_RPC_TRACE")) fprintf(stderr,"dst=%08x src=%08x if=%u\n",ntohl(info.ipi_addr.s_addr),ntohl(peer.sin_addr.s_addr),info.ipi_ifindex);
        uint32_t ip=ntohl(peer.sin_addr.s_addr);
        if(!((ip>>24)==10 || (ip>>20)==0xac1 || (ip>>16)==0xc0a8 || (ip>>16)==0xa9fe || (ip>>24)==127)) continue;
        time_t now=time(NULL); if(now!=second) {second=now;requests=0;} if(++requests>256) continue;
        Slot *slot=NULL;
        for(int i=0;i<slot_count;i++) if(slots[i].address.s_addr==info.ipi_addr.s_addr) slot=&slots[i];
        if(!slot || slot->fault==1) continue;
        size_t length=response(slot,request,(size_t)n,reply,(unsigned)port); if(!length) continue;
        io.iov_base=reply; io.iov_len=length;
        memset(ancillary,0,sizeof(ancillary)); msg.msg_controllen=CMSG_SPACE(sizeof(struct in_pktinfo));
        struct cmsghdr *c=CMSG_FIRSTHDR(&msg); c->cmsg_level=IPPROTO_IP;c->cmsg_type=IP_PKTINFO;c->cmsg_len=CMSG_LEN(sizeof(info));
        info.ipi_ifindex=0; info.ipi_spec_dst=slot->address; memcpy(CMSG_DATA(c),&info,sizeof(info));
        if(sendmsg(sock,&msg,0)<0) perror("RPC reply");
    }
    close(sock); for(int i=0;i<slot_count;i++) close(slots[i].root); return 0;
}
