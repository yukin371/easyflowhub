package relay

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	pathpkg "path"
	"sort"
	"strings"
	"sync"
	"time"

	"scriptmgr/internal/extensions"
)

type Service struct {
	store            *Store
	extensions       *extensions.Registry
	mu               sync.Mutex
	routeCursor      map[string]int
	providerStatuses map[string]*ProviderStatus
	failureThreshold int
}

func NewService(stateDir string) (*Service, error) {
	registry, err := extensions.NewRegistry(stateDir)
	if err != nil {
		return nil, err
	}
	return &Service{
		store:            NewStore(stateDir),
		extensions:       registry,
		routeCursor:      map[string]int{},
		providerStatuses: map[string]*ProviderStatus{},
		failureThreshold: defaultFailThresh,
	}, nil
}

func NewServiceWithDeps(store *Store, registry *extensions.Registry) *Service {
	return &Service{
		store:            store,
		extensions:       registry,
		routeCursor:      map[string]int{},
		providerStatuses: map[string]*ProviderStatus{},
		failureThreshold: defaultFailThresh,
	}
}

func (s *Service) LoadConfig() (Config, error) {
	return s.store.Load()
}

func (s *Service) LoadConfigPath() string {
	return s.store.Path()
}

func (s *Service) SaveConfig(cfg Config) error {
	return s.store.Save(cfg)
}

func (s *Service) ListExtensions() ([]extensions.ListedExtension, []string, error) {
	if s.extensions == nil {
		return nil, nil, nil
	}
	items, err := s.extensions.List()
	return items, s.extensions.Roots(), err
}

func (s *Service) Snapshot() (Snapshot, error) {
	cfg, err := s.LoadConfig()
	if err != nil {
		return Snapshot{}, err
	}

	snapshot := Snapshot{
		Config:    cfg,
		Providers: s.providerSnapshots(cfg),
	}

	if s.extensions != nil {
		snapshot.ExtensionRoots = s.extensions.Roots()
		items, err := s.extensions.List()
		if err != nil {
			snapshot.ExtensionError = err.Error()
		} else {
			snapshot.Extensions = items
		}
	}

	return snapshot, nil
}

func (s *Service) ProxyHandler(stripPrefix string) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		s.handleProxy(w, r, stripPrefix)
	})
}

func (s *Service) handleProxy(w http.ResponseWriter, r *http.Request, stripPrefix string) {
	cfg, err := s.LoadConfig()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	targetPath := normalizeProxyPath(r.URL.Path, stripPrefix)
	body, err := io.ReadAll(r.Body)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{
			"ok":    false,
			"error": "failed to read request body",
		})
		return
	}
	_ = r.Body.Close()
	r.Body = io.NopCloser(bytes.NewReader(body))

	model := extractModel(body)
	route, err := s.resolveRoute(cfg, targetPath, model)
	if err != nil {
		writeJSON(w, http.StatusBadGateway, map[string]any{
			"ok":    false,
			"error": err.Error(),
			"path":  targetPath,
			"model": model,
		})
		return
	}

	excluded := map[string]bool{}
	attempted := []string{}

	for {
		provider, ok := s.pickProvider(cfg, route, model, excluded)
		if !ok {
			writeJSON(w, http.StatusBadGateway, map[string]any{
				"ok":        false,
				"error":     "all upstream providers failed",
				"route_id":  route.ID,
				"attempted": attempted,
				"path":      targetPath,
				"model":     model,
			})
			return
		}

		attempted = append(attempted, provider.ID)
		resp, err := s.forward(r, body, provider, targetPath)
		if err != nil {
			s.recordFailure(provider.ID, 0, err.Error())
			excluded[provider.ID] = true
			continue
		}

		if retryableStatus(resp.StatusCode) {
			s.recordFailure(provider.ID, resp.StatusCode, fmt.Sprintf("upstream status %d", resp.StatusCode))
			excluded[provider.ID] = true
			_ = resp.Body.Close()
			continue
		}

		s.recordSuccess(provider.ID, resp.StatusCode)
		copyResponse(w, resp, provider.ID)
		return
	}
}

func (s *Service) resolveRoute(cfg Config, requestPath, model string) (Route, error) {
	if len(cfg.Routes) == 0 {
		var providerIDs []string
		for _, provider := range cfg.Providers {
			if provider.Enabled {
				providerIDs = append(providerIDs, provider.ID)
			}
		}
		if len(providerIDs) == 0 {
			return Route{}, fmt.Errorf("no enabled relay providers configured")
		}
		return Route{
			ID:           DefaultRouteID,
			Name:         "Default",
			PathPrefixes: []string{"/v1/"},
			ProviderIDs:  providerIDs,
			Strategy:     DefaultStrategy,
		}, nil
	}

	for _, route := range cfg.Routes {
		if routeMatches(route, requestPath, model) {
			return route, nil
		}
	}

	return Route{}, fmt.Errorf("no relay route matched path=%q model=%q", requestPath, model)
}

func routeMatches(route Route, requestPath, model string) bool {
	if len(route.PathPrefixes) > 0 {
		matched := false
		for _, prefix := range route.PathPrefixes {
			if strings.HasPrefix(requestPath, prefix) {
				matched = true
				break
			}
		}
		if !matched {
			return false
		}
	}

	if len(route.ModelPatterns) == 0 || model == "" {
		return true
	}
	return matchesAny(model, route.ModelPatterns)
}

func (s *Service) pickProvider(cfg Config, route Route, model string, excluded map[string]bool) (Provider, bool) {
	candidates := s.candidates(cfg, route, model, excluded)
	if len(candidates) == 0 {
		return Provider{}, false
	}

	expanded := make([]Provider, 0, len(candidates))
	for _, provider := range candidates {
		for i := 0; i < provider.Weight; i++ {
			expanded = append(expanded, provider)
		}
	}

	s.mu.Lock()
	cursor := s.routeCursor[route.ID]
	picked := expanded[cursor%len(expanded)]
	s.routeCursor[route.ID] = (cursor + 1) % len(expanded)
	status := s.ensureStatusLocked(picked.ID)
	now := time.Now().UTC()
	status.LastPickedAt = &now
	s.mu.Unlock()

	return picked, true
}

func (s *Service) candidates(cfg Config, route Route, model string, excluded map[string]bool) []Provider {
	providersByID := map[string]Provider{}
	for _, provider := range cfg.Providers {
		providersByID[provider.ID] = provider
	}

	var candidates []Provider
	for _, providerID := range route.ProviderIDs {
		provider, ok := providersByID[providerID]
		if !ok || !provider.Enabled || excluded[providerID] {
			continue
		}
		if len(provider.ModelPatterns) > 0 && model != "" && !matchesAny(model, provider.ModelPatterns) {
			continue
		}
		candidates = append(candidates, provider)
	}

	if len(candidates) == 0 {
		return nil
	}

	var healthy []Provider
	for _, provider := range candidates {
		if s.statusHealthy(provider.ID) {
			healthy = append(healthy, provider)
		}
	}
	if len(healthy) > 0 {
		return healthy
	}
	return candidates
}

func matchesAny(value string, patterns []string) bool {
	for _, pattern := range patterns {
		pattern = strings.TrimSpace(pattern)
		if pattern == "" {
			continue
		}
		if pattern == "*" {
			return true
		}
		if matched, err := pathpkg.Match(pattern, value); err == nil && matched {
			return true
		}
	}
	return false
}

func (s *Service) forward(original *http.Request, body []byte, provider Provider, targetPath string) (*http.Response, error) {
	baseURL, err := url.Parse(provider.BaseURL)
	if err != nil {
		return nil, fmt.Errorf("invalid provider base_url for %s: %w", provider.ID, err)
	}

	targetURL := *baseURL
	targetURL.Path = joinURLPath(baseURL.Path, targetPath)
	targetURL.RawQuery = original.URL.RawQuery

	req, err := http.NewRequestWithContext(original.Context(), original.Method, targetURL.String(), bytes.NewReader(body))
	if err != nil {
		return nil, err
	}

	copyHeaders(req.Header, original.Header)
	req.Header.Set("X-EasyFlowHub-Relay-Provider", provider.ID)
	if provider.APIKey != "" {
		req.Header.Set("Authorization", "Bearer "+provider.APIKey)
	}
	for key, value := range provider.Headers {
		req.Header.Set(key, value)
	}

	timeout := time.Duration(provider.TimeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = time.Duration(DefaultTimeoutMs) * time.Millisecond
	}

	client := &http.Client{Timeout: timeout}
	return client.Do(req)
}

func (s *Service) providerSnapshots(cfg Config) []ProviderSnapshot {
	s.mu.Lock()
	defer s.mu.Unlock()

	snapshots := make([]ProviderSnapshot, 0, len(cfg.Providers))
	for _, provider := range cfg.Providers {
		status := s.ensureStatusLocked(provider.ID)
		copyStatus := *status
		snapshots = append(snapshots, ProviderSnapshot{
			Provider: provider,
			Status:   copyStatus,
		})
	}

	sort.Slice(snapshots, func(i, j int) bool {
		return snapshots[i].Provider.ID < snapshots[j].Provider.ID
	})

	return snapshots
}

func (s *Service) statusHealthy(providerID string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()

	status := s.ensureStatusLocked(providerID)
	return status.Healthy
}

func (s *Service) recordSuccess(providerID string, statusCode int) {
	s.mu.Lock()
	defer s.mu.Unlock()

	status := s.ensureStatusLocked(providerID)
	now := time.Now().UTC()
	status.Healthy = true
	status.ConsecutiveFailures = 0
	status.LastStatusCode = statusCode
	status.LastError = ""
	status.LastSuccessAt = &now
}

func (s *Service) recordFailure(providerID string, statusCode int, message string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	status := s.ensureStatusLocked(providerID)
	now := time.Now().UTC()
	status.ConsecutiveFailures++
	status.LastStatusCode = statusCode
	status.LastError = message
	status.LastFailureAt = &now
	status.Healthy = status.ConsecutiveFailures < s.failureThreshold
}

func (s *Service) ensureStatusLocked(providerID string) *ProviderStatus {
	status, ok := s.providerStatuses[providerID]
	if !ok {
		status = &ProviderStatus{
			ProviderID: providerID,
			Healthy:    true,
		}
		s.providerStatuses[providerID] = status
	}
	return status
}

func normalizeProxyPath(requestPath, stripPrefix string) string {
	if stripPrefix != "" && strings.HasPrefix(requestPath, stripPrefix) {
		requestPath = strings.TrimPrefix(requestPath, stripPrefix)
	}
	if requestPath == "" {
		return "/"
	}
	if !strings.HasPrefix(requestPath, "/") {
		requestPath = "/" + requestPath
	}
	return requestPath
}

func extractModel(body []byte) string {
	if len(body) == 0 {
		return ""
	}

	var payload struct {
		Model string `json:"model"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		return ""
	}
	return strings.TrimSpace(payload.Model)
}

func retryableStatus(statusCode int) bool {
	return statusCode == http.StatusTooManyRequests || statusCode >= http.StatusInternalServerError
}

func joinURLPath(basePath, requestPath string) string {
	if basePath == "" {
		return requestPath
	}
	if requestPath == "" || requestPath == "/" {
		return basePath
	}
	return strings.TrimRight(basePath, "/") + "/" + strings.TrimLeft(requestPath, "/")
}

func copyHeaders(dst, src http.Header) {
	for key, values := range src {
		for _, value := range values {
			dst.Add(key, value)
		}
	}
}

func copyResponse(w http.ResponseWriter, resp *http.Response, providerID string) {
	defer resp.Body.Close()
	for key, values := range resp.Header {
		for _, value := range values {
			w.Header().Add(key, value)
		}
	}
	w.Header().Set("X-EasyFlowHub-Relay-Provider", providerID)
	w.WriteHeader(resp.StatusCode)
	_, _ = io.Copy(w, resp.Body)
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
