// Static Web Resource Provider v0 — isolated preview, not a production deployment.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log/slog"
	"mime"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"
	"time"
)

const (
	capabilityID      = "web.static-files" // Provisional contract name.
	capabilityVersion = "1.0.0"
)

var validName = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]{0,63}$`)

type resourceConfig struct {
	ID        string `json:"id"`
	Directory string `json:"directory"`
}

type providerConfig struct {
	Listen    string           `json:"listen"`
	CoreURL   string           `json:"core_url"`
	Resources []resourceConfig `json:"resources"`
}

type webBinding struct {
	Resource string `json:"resource"`
	Alias    string `json:"alias"`
}

type instanceWebConfig struct {
	Bindings []webBinding `json:"bindings"`
}

type app struct {
	resources map[string]resourceConfig
	exposed   map[string]string // public alias -> concrete Resource ID
	logger    *slog.Logger
}

func loadJSON(filename string, out any) error {
	f, err := os.Open(filename)
	if err != nil {
		return fmt.Errorf("read %s: %w", filename, err)
	}
	defer f.Close()
	dec := json.NewDecoder(io.LimitReader(f, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(out); err != nil {
		return fmt.Errorf("decode %s: %w", filename, err)
	}
	var extra any
	if err := dec.Decode(&extra); err != io.EOF {
		return fmt.Errorf("%s must contain exactly one JSON value", filename)
	}
	return nil
}

func newApp(cfg providerConfig, web instanceWebConfig, logger *slog.Logger) (*app, error) {
	if logger == nil {
		logger = slog.Default()
	}
	a := &app{
		resources: make(map[string]resourceConfig),
		exposed:   make(map[string]string),
		logger:    logger,
	}
	for _, r := range cfg.Resources {
		if !validName.MatchString(r.ID) {
			return nil, fmt.Errorf("invalid resource ID %q", r.ID)
		}
		if !filepath.IsAbs(r.Directory) || filepath.Clean(r.Directory) != r.Directory {
			return nil, fmt.Errorf("resource %q must have a clean, absolute directory", r.ID)
		}
		if _, exists := a.resources[r.ID]; exists {
			return nil, fmt.Errorf("duplicate resource ID %q", r.ID)
		}
		a.resources[r.ID] = r
	}
	for _, b := range web.Bindings {
		if _, exists := a.resources[b.Resource]; !exists {
			return nil, fmt.Errorf("web binding references unknown resource %q", b.Resource)
		}
		alias := b.Alias
		if alias == "" {
			alias = b.Resource
		}
		if !validName.MatchString(alias) {
			return nil, fmt.Errorf("invalid web alias %q", alias)
		}
		if _, exists := a.exposed[alias]; exists {
			return nil, fmt.Errorf("duplicate public alias %q", alias)
		}
		a.exposed[alias] = b.Resource
	}
	return a, nil
}

// openRoot verifies the configured root exists and is a real directory.
// os.Root.Open confines descendants even when a nested symlink points outside.
func openRoot(dir string) (*os.Root, error) {
	info, err := os.Lstat(dir)
	if err != nil {
		return nil, err
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("configured root is not a directory: %s", dir)
	}
	return os.OpenRoot(dir)
}

func safeRelative(p string) bool {
	if p == "" || strings.HasPrefix(p, "/") {
		return false
	}
	for _, seg := range strings.Split(p, "/") {
		if seg == "" || seg == "." || seg == ".." || strings.HasPrefix(seg, ".") ||
			strings.ContainsAny(seg, "\\\x00") {
			return false
		}
	}
	return true
}

func (a *app) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")

	if r.URL.Path == "/healthz" {
		a.health(w, r)
		return
	}
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		w.Header().Set("Allow", "GET, HEAD")
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	const prefix = "/resource/"
	if !strings.HasPrefix(r.URL.Path, prefix) {
		http.NotFound(w, r)
		return
	}
	pieces := strings.SplitN(strings.TrimPrefix(r.URL.Path, prefix), "/", 2)
	if len(pieces) != 2 || !validName.MatchString(pieces[0]) || !safeRelative(pieces[1]) {
		http.NotFound(w, r)
		return
	}
	id, ok := a.exposed[pieces[0]]
	if !ok {
		http.NotFound(w, r)
		return
	}
	res := a.resources[id]
	root, err := openRoot(res.Directory)
	if err != nil {
		a.logger.Warn("static resource unavailable", "resource_id", id, "error", err)
		w.Header().Set("Retry-After", "5")
		http.Error(w, "resource unavailable", http.StatusServiceUnavailable)
		return
	}
	defer root.Close()

	before, err := root.Lstat(pieces[1])
	if err != nil || !before.Mode().IsRegular() {
		http.NotFound(w, r) // Reject symlinks and special files before opening.
		return
	}
	// Nonblocking open protects against FIFO replacement between Lstat and Open.
	file, err := root.OpenFile(pieces[1], os.O_RDONLY|syscall.O_NONBLOCK, 0)
	if err != nil {
		http.NotFound(w, r) // Do not reveal filesystem details.
		return
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() {
		http.NotFound(w, r) // Never list directories or serve special files.
		return
	}
	if contentType := mime.TypeByExtension(path.Ext(pieces[1])); contentType != "" {
		w.Header().Set("Content-Type", contentType)
	}
	// Zero modification time deliberately disables conditional 304 responses.
	// Combined with no-store, each request can read updated contents.
	http.ServeContent(w, r, info.Name(), time.Time{}, file)
}

func (a *app) health(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		w.Header().Set("Allow", "GET, HEAD")
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	readyCount := 0
	for _, res := range a.resources {
		root, err := openRoot(res.Directory)
		if err == nil {
			readyCount++
			_ = root.Close()
		}
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]any{
		"status":                "running",
		"ready_resources":       readyCount,
		"unavailable_resources": len(a.resources) - readyCount,
	})
}

func (a *app) reconcile(ctx context.Context, coreURL string) {
	coreURL = strings.TrimRight(coreURL, "/")
	client := &http.Client{Timeout: 3 * time.Second}
	for id, r := range a.resources {
		root, err := openRoot(r.Directory)
		if err != nil {
			a.logger.Warn("skipping unavailable resource registration", "resource_id", id, "error", err)
			continue
		}
		_ = root.Close()

		url := coreURL + "/resources/" + id
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		if err != nil {
			a.logger.Error("invalid Core URL", "error", err)
			continue
		}
		resp, err := client.Do(req)
		if err != nil {
			a.logger.Warn("Core lookup failed", "resource_id", id, "error", err)
			continue
		}
		_, _ = io.Copy(io.Discard, resp.Body)
		_ = resp.Body.Close()
		if resp.StatusCode == http.StatusOK {
			continue
		}
		if resp.StatusCode != http.StatusNotFound {
			a.logger.Warn("Core lookup unexpected status", "resource_id", id, "status", resp.StatusCode)
			continue
		}

		payload := map[string]any{
			"id": id, "name": id, "type": "static-web",
			"description": "Static Web Resource registered by Go Provider v0",
			"provides": map[string]any{"capabilities": []map[string]string{
				{"id": capabilityID, "version": capabilityVersion},
			}},
		}
		body, err := json.Marshal(payload)
		if err != nil {
			a.logger.Error("marshal registration", "error", err)
			continue
		}
		req, err = http.NewRequestWithContext(ctx, http.MethodPost, coreURL+"/resources", strings.NewReader(string(body)))
		if err != nil {
			a.logger.Error("invalid Core URL", "error", err)
			continue
		}
		req.Header.Set("Content-Type", "application/json")
		resp, err = client.Do(req)
		if err != nil {
			a.logger.Warn("Core registration failed", "resource_id", id, "error", err)
			continue
		}
		_, _ = io.Copy(io.Discard, resp.Body)
		_ = resp.Body.Close()
		if resp.StatusCode == http.StatusConflict {
			a.logger.Warn("Core registration conflict; not claiming ownership", "resource_id", id)
			continue
		}
		if resp.StatusCode != http.StatusCreated {
			a.logger.Warn("Core registration rejected", "resource_id", id, "status", resp.StatusCode)
			continue
		}
		a.logger.Info("Core resource registered", "resource_id", id)
	}
}

func (a *app) runReconciler(ctx context.Context, coreURL string) {
	a.reconcile(ctx, coreURL)
	ticker := time.NewTicker(15 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			a.reconcile(ctx, coreURL)
		}
	}
}

func run() error {
	providerFile := flag.String("provider-config", "", "JSON provider resource configuration")
	webFile := flag.String("web-bindings", "", "JSON explicit Instance public aliases")
	flag.Parse()
	if *providerFile == "" || *webFile == "" {
		return errors.New("both -provider-config and -web-bindings are required")
	}
	var cfg providerConfig
	var web instanceWebConfig
	if err := loadJSON(*providerFile, &cfg); err != nil {
		return err
	}
	if err := loadJSON(*webFile, &web); err != nil {
		return err
	}
	if cfg.Listen == "" {
		cfg.Listen = "127.0.0.1:18580"
	}
	if cfg.CoreURL != "" && !(strings.HasPrefix(cfg.CoreURL, "http://") || strings.HasPrefix(cfg.CoreURL, "https://")) {
		return errors.New("core_url must start with http:// or https://")
	}
	app, err := newApp(cfg, web, slog.Default())
	if err != nil {
		return err
	}
	for id, r := range app.resources {
		root, err := openRoot(r.Directory)
		if err != nil {
			app.logger.Warn("configured resource unavailable", "resource_id", id, "error", err)
		} else {
			_ = root.Close()
			app.logger.Info("resource ready", "resource_id", id)
		}
	}
	listener, err := net.Listen("tcp", cfg.Listen)
	if err != nil {
		return err
	}
	defer listener.Close()
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if cfg.CoreURL != "" {
		go app.runReconciler(ctx, cfg.CoreURL)
	}
	server := &http.Server{Handler: app, ReadHeaderTimeout: 5 * time.Second}
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()
	app.logger.Info("Static Web Provider listening", "addr", listener.Addr().String(), "exposures", len(app.exposed))
	err = server.Serve(listener)
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func main() {
	if err := run(); err != nil {
		slog.Error("Static Web Provider stopped", "error", err)
		os.Exit(1)
	}
}
