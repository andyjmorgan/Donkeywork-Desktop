package main

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os/exec"
	"path/filepath"
	"strconv"
	"sync"
	"time"
)

// Lab-only endpoint, enabled explicitly by the trusted managed-session launcher.
// Identity remains the service UID; request bodies never choose an account/path.
func managedResize(root string) http.HandlerFunc {
	var mu sync.Mutex
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			w.WriteHeader(405)
			return
		}
		var request struct {
			Width  int `json:"width"`
			Height int `json:"height"`
		}
		d := json.NewDecoder(http.MaxBytesReader(w, r.Body, 1024))
		d.DisallowUnknownFields()
		if d.Decode(&request) != nil || d.Decode(new(any)) != io.EOF ||
			!((request.Width == 1920 && request.Height == 1080) || (request.Width == 3840 && request.Height == 2160)) {
			http.Error(w, "Unsupported pilot mode", 400)
			return
		}
		if !mu.TryLock() {
			http.Error(w, "Resize already active", 409)
			return
		}
		defer mu.Unlock()
		ctx, cancel := context.WithTimeout(r.Context(), 8*time.Second)
		defer cancel()
		command := exec.CommandContext(ctx, "python3", filepath.Join(root, "web.py"), root,
			"--resize", strconv.Itoa(request.Width), strconv.Itoa(request.Height))
		result, err := command.Output()
		if err != nil {
			http.Error(w, "Resize unavailable; release input and retry", 409)
			return
		}
		if len(result) > 4096 || !json.Valid(result) {
			http.Error(w, "Invalid worker response", 502)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(result)
	}
}
