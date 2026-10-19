package localapi

import (
	"net"
	"net/http"
)

const LoopbackHost = "127.0.0.1"

var allowedOrigins = map[string]struct{}{
	"http://localhost:1420":  {},
	"http://127.0.0.1:1420":  {},
	"http://tauri.localhost": {},
	"tauri://localhost":      {},
}

func LoopbackAddress(port string) string {
	return net.JoinHostPort(LoopbackHost, port)
}

func IsLoopbackAddress(addr string) bool {
	host, _, err := net.SplitHostPort(addr)
	if err != nil {
		return false
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

func WithBrowserOrigins(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		origin := r.Header.Get("Origin")
		if origin != "" {
			if _, ok := allowedOrigins[origin]; !ok {
				http.Error(w, "origin not allowed", http.StatusForbidden)
				return
			}

			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Add("Vary", "Origin")
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, PUT, DELETE, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
			if r.Method == http.MethodOptions {
				w.WriteHeader(http.StatusNoContent)
				return
			}
		}

		next.ServeHTTP(w, r)
	})
}
