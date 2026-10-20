package main

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

func testApp(t *testing.T, resources []resourceConfig, bindings []webBinding) *app {
	t.Helper()
	a, err := newApp(providerConfig{Resources: resources}, instanceWebConfig{Bindings: bindings},
		slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		t.Fatal(err)
	}
	return a
}

func serve(a *app, method, url string) *httptest.ResponseRecorder {
	w := httptest.NewRecorder()
	a.ServeHTTP(w, httptest.NewRequest(method, url, nil))
	return w
}

func TestLiveFilesAndExplicitAliases(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "theme.css"), []byte("body{color:red}"), 0644); err != nil {
		t.Fatal(err)
	}
	a := testApp(t, []resourceConfig{{ID: "shared-assets", Directory: dir}},
		[]webBinding{{Resource: "shared-assets", Alias: "icons"}})
	w := serve(a, "GET", "/resource/icons/theme.css")
	if w.Code != 200 || w.Body.String() != "body{color:red}" {
		t.Fatalf("initial: %d %q", w.Code, w.Body.String())
	}
	if got := w.Header().Get("Cache-Control"); got != "no-store" {
		t.Fatalf("cache header: %q", got)
	}
	if got := w.Header().Get("X-Content-Type-Options"); got != "nosniff" {
		t.Fatalf("nosniff: %q", got)
	}
	if w := serve(a, "GET", "/resource/shared-assets/theme.css"); w.Code != 404 {
		t.Fatalf("unpublished resource ID was visible: %d", w.Code)
	}
	if err := os.WriteFile(filepath.Join(dir, "theme.css"), []byte("body{color:blue}"), 0644); err != nil {
		t.Fatal(err)
	}
	if w := serve(a, "GET", "/resource/icons/theme.css"); w.Code != 200 || w.Body.String() != "body{color:blue}" {
		t.Fatalf("changed file not visible: %d %q", w.Code, w.Body.String())
	}
	if err := os.WriteFile(filepath.Join(dir, "new.js"), []byte("alert(1)"), 0644); err != nil {
		t.Fatal(err)
	}
	if w := serve(a, "GET", "/resource/icons/new.js"); w.Code != 200 || w.Body.String() != "alert(1)" {
		t.Fatalf("new file not visible: %d %q", w.Code, w.Body.String())
	}
	if w := serve(a, "GET", "/resource/icons/absent.css"); w.Code != 404 {
		t.Fatalf("missing file: %d", w.Code)
	}
	if w := serve(a, "GET", "/resource/icons/"); w.Code != 404 {
		t.Fatalf("directory listing: %d", w.Code)
	}
	if w := serve(a, "HEAD", "/resource/icons/theme.css"); w.Code != 200 || w.Body.Len() != 0 {
		t.Fatalf("HEAD: %d %d", w.Code, w.Body.Len())
	}
	if w := serve(a, "POST", "/resource/icons/theme.css"); w.Code != 405 {
		t.Fatalf("POST: %d", w.Code)
	}
}

func TestPathRestrictions(t *testing.T) {
	parent := t.TempDir()
	dir := filepath.Join(parent, "public")
	if err := os.Mkdir(dir, 0755); err != nil {
		t.Fatal(err)
	}
	_ = os.WriteFile(filepath.Join(parent, "secret.txt"), []byte("secret"), 0644)
	_ = os.WriteFile(filepath.Join(dir, ".env"), []byte("no"), 0644)
	_ = os.Mkdir(filepath.Join(dir, "sub"), 0755)
	_ = os.Symlink(filepath.Join(parent, "secret.txt"), filepath.Join(dir, "linked.txt"))
	if err := syscall.Mkfifo(filepath.Join(dir, "pipe"), 0644); err != nil {
		t.Fatal(err)
	}
	a := testApp(t, []resourceConfig{{ID: "a", Directory: dir}},
		[]webBinding{{Resource: "a"}})
	for _, path := range []string{
		"/resource/a/../secret.txt",
		"/resource/a/%2e%2e/secret.txt",
		"/resource/a/.env",
		"/resource/a/sub/",
		"/resource/a/linked.txt",
		"/resource/a/pipe",
		"/resource/unknown/secret.txt",
		"/resource/a/sub/../../secret.txt",
	} {
		w := serve(a, "GET", path)
		if w.Code != 404 {
			t.Fatalf("%s exposed content (status %d): %s", path, w.Code, w.Body.String())
		}
	}
}

func TestResourceUnavailableIsIsolated(t *testing.T) {
	parent := t.TempDir()
	okDir := filepath.Join(parent, "ok")
	missing := filepath.Join(parent, "missing")
	if err := os.Mkdir(okDir, 0755); err != nil {
		t.Fatal(err)
	}
	_ = os.WriteFile(filepath.Join(okDir, "ok.txt"), []byte("alive"), 0644)
	a := testApp(t,
		[]resourceConfig{{ID: "ok", Directory: okDir}, {ID: "missing", Directory: missing}},
		[]webBinding{{Resource: "ok"}, {Resource: "missing"}})
	if w := serve(a, "GET", "/resource/missing/foo.css"); w.Code != 503 {
		t.Fatalf("missing root status: %d", w.Code)
	}
	if w := serve(a, "GET", "/resource/ok/ok.txt"); w.Code != 200 {
		t.Fatalf("healthy resource affected: %d", w.Code)
	}
	if err := os.Mkdir(missing, 0755); err != nil {
		t.Fatal(err)
	}
	_ = os.WriteFile(filepath.Join(missing, "foo.css"), []byte("new"), 0644)
	if w := serve(a, "GET", "/resource/missing/foo.css"); w.Code != 200 {
		t.Fatalf("restored root status: %d", w.Code)
	}
	if err := os.RemoveAll(missing); err != nil {
		t.Fatal(err)
	}
	if w := serve(a, "GET", "/resource/missing/foo.css"); w.Code != 503 {
		t.Fatalf("deleted root status: %d", w.Code)
	}
}

func TestConfigValidation(t *testing.T) {
	root := t.TempDir()
	cases := []struct {
		name      string
		resources []resourceConfig
		bindings  []webBinding
	}{
		{"relative", []resourceConfig{{"a", "./a"}}, nil},
		{"duplicate-id", []resourceConfig{{"a", root}, {"a", root}}, nil},
		{"unknown-binding", []resourceConfig{{"a", root}}, []webBinding{{Resource: "b"}}},
		{"duplicate-alias", []resourceConfig{{"a", root}, {"b", root}},
			[]webBinding{{Resource: "a", Alias: "common"}, {Resource: "b", Alias: "common"}}},
		{"bad-alias", []resourceConfig{{"a", root}}, []webBinding{{Resource: "a", Alias: "../etc"}}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := newApp(providerConfig{Resources: tc.resources},
				instanceWebConfig{Bindings: tc.bindings}, nil); err == nil {
				t.Fatal("expected config rejection")
			}
		})
	}
}

func TestCoreResourceRegistration(t *testing.T) {
	good := t.TempDir()
	missing := filepath.Join(t.TempDir(), "absent")
	var gotID string
	var capID string
	var postCount int
	core := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == "GET" {
			http.NotFound(w, r)
			return
		}
		if r.Method != "POST" || r.URL.Path != "/resources" {
			t.Errorf("unexpected Core request: %s %s", r.Method, r.URL.Path)
			w.WriteHeader(400)
			return
		}
		postCount++
		var data struct {
			ID       string `json:"id"`
			Provides struct {
				Capabilities []struct {
					ID string `json:"id"`
				} `json:"capabilities"`
			} `json:"provides"`
		}
		if err := json.NewDecoder(r.Body).Decode(&data); err != nil {
			t.Error(err)
		}
		gotID = data.ID
		if len(data.Provides.Capabilities) == 1 {
			capID = data.Provides.Capabilities[0].ID
		}
		w.WriteHeader(201)
	}))
	defer core.Close()
	a := testApp(t, []resourceConfig{{ID: "yes", Directory: good}, {ID: "no", Directory: missing}}, nil)
	a.reconcile(context.Background(), core.URL)
	if gotID != "yes" || capID != capabilityID || postCount != 1 {
		t.Fatalf("registration: id=%q capability=%q posts=%d", gotID, capID, postCount)
	}
}

func TestJSONRejectsUnknownFields(t *testing.T) {
	file := filepath.Join(t.TempDir(), "bad.json")
	_ = os.WriteFile(file, []byte(`{"listen":"127.0.0.1:1000","unknown":true}`), 0644)
	var cfg providerConfig
	if err := loadJSON(file, &cfg); err == nil || !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("expected strict decoder error, got %v", err)
	}
}
