package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestPrivateRestoreFile(t *testing.T) {
	dir := t.TempDir()
	if err := os.Chmod(dir, 0700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "restore")
	if err := saveToken(path, "first"); err != nil {
		t.Fatal(err)
	}
	if err := saveToken(path, "replacement"); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil || string(data) != "replacement" {
		t.Fatal("token not replaced")
	}
	info, _ := os.Stat(path)
	if info.Mode().Perm() != 0600 {
		t.Fatal("token is not private")
	}
	if err := os.Chmod(dir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := saveToken(path, "unsafe"); err == nil {
		t.Fatal("accepted public directory")
	}
}
