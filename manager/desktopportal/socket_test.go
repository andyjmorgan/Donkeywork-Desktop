package desktopportal

import (
	"net"
	"os"
	"path/filepath"
	"testing"
)

func TestSocketRecovery(t *testing.T) {
	path := filepath.Join(t.TempDir(), "console.sock")
	if err := os.WriteFile(path, []byte("keep"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := listenUnix(path); err == nil {
		t.Fatal("replaced regular file")
	}
	os.Remove(path)
	first, err := listenUnix(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := listenUnix(path); err == nil {
		t.Fatal("replaced active socket")
	}
	first.(*net.UnixListener).SetUnlinkOnClose(false)
	first.Close()
	second, err := listenUnix(path)
	if err != nil {
		t.Fatal(err)
	}
	second.Close()
}
