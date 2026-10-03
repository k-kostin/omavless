//go:build p4_cookie_transport

// SPDX-License-Identifier: MIT
package main

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestCookieModeInput(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "cookie.mode")
	for _, test := range []struct {
		payload           string
		accepted, corrupt bool
	}{
		{"pass\n", true, false}, {"corrupt\n", true, true},
		{"pass", false, false}, {" pass\n", false, false}, {"secret-canary\n", false, false}, {"corrupt\nextra", false, false},
	} {
		if os.WriteFile(path, []byte(test.payload), 0600) != nil {
			t.Fatal("synthetic mode write failed")
		}
		corrupt, err := fixtureMode(root)
		if (err == nil) != test.accepted || err == nil && corrupt != test.corrupt {
			t.Fatal("fixed mode admission mismatch")
		}
	}
	if os.WriteFile(path, []byte("pass\n"), 0600) != nil || os.Chmod(path, 0644) != nil {
		t.Fatal("synthetic mode permissions failed")
	}
	if _, err := fixtureMode(root); err == nil {
		t.Fatal("public mode file accepted")
	}
	if os.Chmod(path, 0600) != nil || os.Link(path, filepath.Join(root, "alias")) != nil {
		t.Fatal("synthetic mode link failed")
	}
	if _, err := fixtureMode(root); err == nil {
		t.Fatal("multiply linked mode file accepted")
	}
	if os.Remove(path) != nil || os.Symlink(filepath.Join(root, "alias"), path) != nil {
		t.Fatal("synthetic symlink failed")
	}
	if _, err := fixtureMode(root); err == nil {
		t.Fatal("symlink mode file accepted")
	}
	if os.Remove(path) != nil || syscall.Mkfifo(path, 0600) != nil {
		t.Fatal("synthetic FIFO failed")
	}
	if _, err := fixtureMode(root); err == nil {
		t.Fatal("FIFO mode file accepted")
	}
}
