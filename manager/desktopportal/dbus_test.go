package desktopportal

import (
	"bufio"
	"context"
	"errors"
	"github.com/godbus/dbus/v5"
	"os/exec"
	"strings"
	"testing"
	"time"
)

type fakePortal struct {
	conn    *dbus.Conn
	respond bool
	closed  chan struct{}
}
type fakeRequest struct{ closed chan struct{} }

func (r *fakeRequest) Close() *dbus.Error {
	select {
	case r.closed <- struct{}{}:
	default:
	}
	return nil
}
func (f *fakePortal) Start(sender dbus.Sender, options Options) (dbus.ObjectPath, *dbus.Error) {
	token := options["handle_token"].Value().(string)
	owner := strings.ReplaceAll(strings.TrimPrefix(string(sender), ":"), ".", "_")
	handle := dbus.ObjectPath("/org/freedesktop/portal/desktop/request/" + owner + "/" + token)
	f.conn.Export(&fakeRequest{f.closed}, handle, "org.freedesktop.portal.Request")
	// Intentionally emit before returning the method result: subscribing late
	// would lose this success response and leave the client waiting forever.
	if f.respond {
		f.conn.Emit(handle, "org.freedesktop.portal.Request.Response", uint32(0), Options{})
	}
	return handle, nil
}
func TestRequestEarlySignalAndCancellation(t *testing.T) {
	binary, e := exec.LookPath("dbus-daemon")
	if e != nil {
		t.Skip("dbus-daemon not installed")
	}
	cmd := exec.Command(binary, "--session", "--nofork", "--print-address=1")
	stdout, e := cmd.StdoutPipe()
	if e != nil {
		t.Fatal(e)
	}
	if e = cmd.Start(); e != nil {
		t.Fatal(e)
	}
	defer func() { cmd.Process.Kill(); cmd.Wait() }()
	address, e := bufio.NewReader(stdout).ReadString('\n')
	if e != nil {
		t.Fatal(e)
	}
	server, e := dbus.Connect(strings.TrimSpace(address))
	if e != nil {
		t.Fatal(e)
	}
	defer server.Close()
	if _, e = server.RequestName(destination, dbus.NameFlagDoNotQueue); e != nil {
		t.Fatal(e)
	}
	client, e := dbus.Connect(strings.TrimSpace(address))
	if e != nil {
		t.Fatal(e)
	}
	defer client.Close()
	c := &Client{client}
	for _, reply := range []bool{true, false} {
		closed := make(chan struct{}, 1)
		if e = server.Export(&fakePortal{server, reply, closed}, path, remote); e != nil {
			t.Fatal(e)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 300*time.Millisecond)
		_, e = c.request(ctx, remote+".Start", nil, Options{})
		cancel()
		if reply && e != nil {
			t.Fatal(e)
		}
		if !reply && !errors.Is(e, context.DeadlineExceeded) {
			t.Fatalf("expected timeout, got %v", e)
		}
		select {
		case <-closed:
		case <-time.After(time.Second):
			t.Fatal("request was not closed")
		}
	}
}
