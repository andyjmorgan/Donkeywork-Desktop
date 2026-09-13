package desktopportal

import (
	"errors"
	"net"
	"os"
	"syscall"
	"time"
)

func listenUnix(path string) (net.Listener, error) {
	if info, err := os.Lstat(path); err == nil {
		stat, ok := info.Sys().(*syscall.Stat_t)
		if info.Mode()&os.ModeSocket == 0 || !ok || stat.Uid != uint32(os.Geteuid()) {
			return nil, errors.New("refusing to replace unowned or non-socket endpoint")
		}
		conn, err := net.DialTimeout("unix", path, time.Second)
		if err == nil {
			conn.Close()
			return nil, errors.New("console socket already active")
		}
		if !errors.Is(err, syscall.ECONNREFUSED) {
			return nil, err
		}
		if err = os.Remove(path); err != nil {
			return nil, err
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	return net.Listen("unix", path)
}
