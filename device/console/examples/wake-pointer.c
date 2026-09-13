/* Bounded wake probe: a relative mouse moves one unit and immediately back. */
#include <linux/uinput.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include <string.h>
static int emit(int fd, unsigned short type, unsigned short code, int value) {
    struct input_event e = {0}; e.type=type; e.code=code; e.value=value;
    return write(fd, &e, sizeof(e)) == sizeof(e) ? 0 : -1;
}
int main(void) {
    int fd=open("/dev/uinput", O_WRONLY|O_CLOEXEC);
    if(fd<0) {perror("uinput");return 1;}
    struct uinput_setup setup={0};
    setup.id.bustype=BUS_VIRTUAL;
    snprintf(setup.name, sizeof(setup.name), "DonkeyWork bounded wake probe");
    if(ioctl(fd, UI_SET_EVBIT, EV_KEY)<0 || ioctl(fd, UI_SET_KEYBIT, BTN_LEFT)<0 ||
       ioctl(fd, UI_SET_EVBIT, EV_REL)<0 || ioctl(fd, UI_SET_RELBIT, REL_X)<0 ||
       ioctl(fd, UI_SET_RELBIT, REL_Y)<0 || ioctl(fd, UI_DEV_SETUP, &setup)<0 ||
       ioctl(fd, UI_DEV_CREATE)<0) {perror("create");close(fd);return 1;}
    /* Probe-only settle; production must verify udev/libinput readiness. */
    sleep(1);
    int failed=emit(fd,EV_REL,REL_X,1) || emit(fd,EV_SYN,SYN_REPORT,0);
    usleep(100000);
    failed=failed || emit(fd,EV_REL,REL_X,-1) || emit(fd,EV_SYN,SYN_REPORT,0);
    usleep(100000);
    ioctl(fd,UI_DEV_DESTROY); close(fd);
    return failed ? 1 : 0;
}
